mod generic;
mod utils;
pub mod genesis;
use sp_api::ProvideRuntimeApi;
use std::sync::Arc;
use elgamal_wasm::generic::PublicKey;
use elgamal_wasm::{KeyGenerator, RawPublicKey};
use rug::{integer::Order, rand::RandState, Complete, Integer};
use sc_consensus_pow::{Error, PowAlgorithm};
use sha2::{Digest, Sha256};
use sp_consensus_pow::{DifficultyApi, Seal as RawSeal};
use sp_core::{H256, U256};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use codec::{Encode, Decode};
use cp_constants::{Difficulty};
use sc_client_api::{backend::AuxStore, blockchain::HeaderBackend};

// local packages.
pub use crate::generic::{CycleFinding, Hash, MapResult, Mapping, MappingError, Solution, State, Solutions};
use utils::{bigint_u256, u256_bigint, gen_bigint_range};

// constants.
const BIG_INT_0: Integer = Integer::ZERO;

/// A Seal struct that will be encoded to a Vec<u8> as used as the
/// `RawSeal` type.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Seal {
	pub difficulty: Difficulty,
	pub pubkey: RawPublicKey,
	pub solutions: Solutions<U256>,
	pub nonce: U256,
}

impl Seal {
	pub fn try_cpu_mining<C: Clone + Hash<Integer, U256>>(&self, compute: &mut C, seed: U256) -> Option<Self>{
		let seed_int = u256_bigint(&seed);
		let old_pubkey = &self.pubkey;
		// generate a new pubkey from existing pubkey with difficulty adjustment.
		// TODO: difficulty adjustment is not yet implemented.
		let difficulty = self.difficulty;
		let raw_pubkey = old_pubkey.yield_pubkey(difficulty as u32);
		let pubkey = PublicKey::<Integer>::from_raw(raw_pubkey.clone());
		if let Some(solutions) = pollard_rho(pubkey.clone(), compute, seed_int) {
			// if find the solutions, build a new seal.
			Some(Seal {
				difficulty,
				pubkey: pubkey.to_raw(),
				solutions: (solutions.0.to_u256(), solutions.1.to_u256()),
				nonce: compute.get_nonce(),
			})
		} else{
			None
		}
	}
}

/// Determine whether the given hash satisfies the given difficulty.
/// The test is done by multiplying the two together. If the product
/// overflows the bounds of U128, then the product (and thus the hash)
/// was too high.
// fn hash_meets_difficulty(seal_difficulty: &Difficulty, difficulty: Difficulty) -> bool {
// 	seal_difficulty == &difficulty
// }

/// A not-yet-computed attempt to solve the proof of work. Calling the
/// compute method will compute the hash and return the seal.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Compute {
	pub difficulty: Difficulty,
	pub pre_hash: H256,
	pub nonce: U256,
}

impl Solution<Integer> {
	fn new_random(n: Integer, seed: &Integer) -> Self {
		let mut rand = RandState::new_mersenne_twister();
		rand.seed(seed);
		Solution {
			a: gen_bigint_range(&mut rand, &BIG_INT_0, &n),
			b: gen_bigint_range(&mut rand, &BIG_INT_0, &n),
			n,
		}
	}

	fn to_u256(&self) -> Solution<U256> {
		Solution::<U256> {
			a: bigint_u256(&self.a),
			b: bigint_u256(&self.b),
			n: bigint_u256(&self.n),
		}
	}

	fn from_u256(solution: &Solution<U256>) -> Self {
		Solution::<Integer> {
			a: u256_bigint(&solution.a),
			b: u256_bigint(&solution.b),
			n: u256_bigint(&solution.n),
		}
	}
}

impl State<Integer> {
	/// Derive a new node state from a public key.
	fn from_pub_key(key: PublicKey<Integer>, seed: Integer) -> Self {
		let p_1 = Integer::from(&key.p - 1);
		let n = Integer::from(&p_1 / 2);
		let solution = Solution::new_random(n, &seed);
		let g_a_p = Integer::from(key.g.pow_mod_ref(&solution.a, &key.p).unwrap());
		let h_b_p = Integer::from(key.h.pow_mod_ref(&solution.b, &key.p).unwrap());
		let y = Integer::from(g_a_p * h_b_p).div_rem_euc(key.p.clone()).1;
		// NOTE: never use 0 to initialize integers which may lead to memory corruption.
		State::<Integer> { solution, nonce: Integer::from(1), work: y, pubkey: key }
	}
}

impl Hash<Integer, U256> for Compute {
	fn set_nonce(&mut self, int: &Integer) {
		self.nonce = bigint_u256(int);
	}

	fn get_nonce(&self) -> U256 { self.nonce.clone()}

	fn hash_integer(&self) -> Integer {
		// digest nonce by hashing with header data.
		let data = &self.encode()[..];
		let mut hasher = Sha256::new();
		hasher.update(&data);
		// convert hash results to integer in little endian order.
		Integer::from_digits(hasher.finalize().as_slice(), Order::Lsf)
	}
}

impl Mapping<Integer> for State<Integer> {
	/// The pollard rho miner with a mapping function which is hard to compute reversely.
	fn func_f(&self, x_i: &Integer, y_i: &Integer) -> MapResult<Integer> {
		let base = &self.pubkey.g;
		let h = &self.pubkey.h;
		let p = &self.pubkey.p;
		match x_i.mod_u(3) {
			0 => Ok(Integer::from(y_i.pow_mod_ref(x_i, p).unwrap())),
			1 => {
				let base_hash_p = Integer::from(base.pow_mod_ref(x_i, p).unwrap());
				Ok((base_hash_p * y_i).div_rem_euc_ref(p).complete().1)
			},
			2 => {
				let h_hash_p = Integer::from(h.pow_mod_ref(x_i, p).unwrap());
				Ok((h_hash_p * y_i).div_rem_euc_ref(p).complete().1)
			},
			_ => Err(MappingError),
		}
	}

	fn func_g(&self, a_i: &Integer, x_i: &Integer) -> MapResult<Integer> {
		let p_1 = Integer::from(&self.pubkey.p - 1);
		let a_m_x = (a_i * x_i).complete();
		let a_p_x = (a_i + x_i).complete();
		match x_i.mod_u(3) {
			0 => Ok(a_m_x.div_rem_euc_ref(&p_1).complete().1),
			1 => Ok(a_p_x.div_rem_euc_ref(&p_1).complete().1),
			2 => Ok(a_i.clone()),
			_ => Err(MappingError),
		}
	}

	fn func_h(&self, b_i: &Integer, x_i: &Integer) -> MapResult<Integer> {
		let p_1 = Integer::from(&self.pubkey.p - 1);
		let b_m_x = (b_i * x_i).complete();
		let b_p_x = (b_i + x_i).complete();
		match x_i.mod_u(3) {
			0 => Ok(b_m_x.div_rem_euc_ref(&p_1).complete().1),
			1 => Ok(b_i.clone()),
			2 => Ok(b_p_x.div_rem_euc_ref(&p_1).complete().1),
			_ => Err(MappingError),
		}
	}
}

impl CycleFinding<Integer, U256> for State<Integer> {
	/// Single Step Transition between states calculating with hashable data.
	fn transit<C: Hash<Integer, U256>>(self, hashable: &mut C) -> MapResult<State<Integer>> {
		hashable.set_nonce(&self.work);
		let raw_int = hashable.hash_integer();
		let hash_i = raw_int.div_rem_euc(self.pubkey.p.clone()).1;
		let work = self.func_f(&hash_i, &self.work)?;
		let a = self.func_g(&self.solution.a, &hash_i)?;
		let b = self.func_h(&self.solution.b, &hash_i)?;
		Ok(State::<Integer> {
			solution: Solution { a, b, n: self.solution.n },
			work,
			nonce: self.work,
			pubkey: self.pubkey,
		})
	}
}

/// To and from raw bytes of a public key. Use little endian byte order by default.
pub trait RawKey {
	fn to_raw(self) -> RawPublicKey;
	fn from_raw(raw_key: RawPublicKey) -> Self;
}

impl RawKey for PublicKey<Integer> {
	fn to_raw(self) -> RawPublicKey {
		RawPublicKey {
			p: bigint_u256(&self.p),
			g: bigint_u256(&self.g),
			h: bigint_u256(&self.h),
			bit_length: self.bit_length,
		}
	}

	fn from_raw(raw_key: RawPublicKey) -> Self {
		PublicKey::<Integer>{
			p: u256_bigint(&raw_key.p),
			g: u256_bigint(&raw_key.g),
			h: u256_bigint(&raw_key.h),
			bit_length: raw_key.bit_length,
		}
	}
}

/// A verifier contains methods to validate mining results.
pub struct SolutionVerifier;

impl SolutionVerifier {
	/// Derive one side of the value for the equation in the pollard rho method.
	fn derive(&self, solution: &Solution<Integer>, pubkey: &PublicKey<Integer>) -> Integer {
		let g_a_p = Integer::from(pubkey.g.pow_mod_ref(&solution.a, &pubkey.p).unwrap());
		let h_b_p = Integer::from(pubkey.h.pow_mod_ref(&solution.b, &pubkey.p).unwrap());
		(g_a_p * h_b_p).div_rem_euc_ref(&pubkey.p).complete().1
	}

	/// Verify the validation of solutions and
	fn verify(&self, solutions: &Solutions<Integer>, pubkey: &PublicKey<Integer>, header: &Compute) -> bool {
		let y_1 = self.derive(&solutions.0, &pubkey);
		let y_2 =  self.derive(&solutions.1, &pubkey);
		// if solutions are valid, verify the hash of nonce.
		let hash_i = header.hash_integer().div_rem_euc(pubkey.p.clone()).1;
		let nonce = u256_bigint(&header.nonce);
		let state = State::<Integer>::from_pub_key(pubkey.clone(), Integer::from(1));
		let work = state.func_f(&hash_i, &nonce).unwrap();
		y_1 == y_2 && y_1 == work
	}
}

/// A minimal PoW algorithm that uses pollard rho method.
/// Difficulty is fixed at 48 bit long uint.
#[derive(Clone)]
pub struct MinimalCapsuleAlgorithm;

// Here we implement the minimal Capsule Pow Algorithm trait
impl<B: BlockT<Hash = H256>> PowAlgorithm<B> for MinimalCapsuleAlgorithm {
	type Difficulty = Difficulty;

	fn difficulty(&self, _parent: B::Hash) -> Result<Self::Difficulty, Error<B>> {
		// Fixed difficulty hardcoded here
		Ok(48 as Difficulty)
	}

	fn verify(
		&self,
		_parent: &BlockId<B>,
		pre_hash: &H256,
		_pre_digest: Option<&[u8]>,
		seal: &RawSeal,
		difficulty: Self::Difficulty,
	) -> Result<bool, Error<B>> {
		// Try to construct a seal object by decoding the raw seal given
		let seal = match Seal::decode(&mut &seal[..]) {
			Ok(seal) => seal,
			Err(_) => return Ok(false),
		};

		// Make sure the provided work actually comes from the correct pre_hash
		let header = Compute {
			difficulty,
			pre_hash: *pre_hash,
			nonce: seal.nonce,
		};
		let raw_key = seal.pubkey;
		let pubkey = PublicKey::<Integer>::from_raw(raw_key);
		let verifier = SolutionVerifier;
		let solutions = (Solution::<Integer>::from_u256(&seal.solutions.0),
						 Solution::<Integer>::from_u256(&seal.solutions.1));
		if verifier.verify(&solutions, &pubkey, &header) {
			return Ok(true);
		}

		Ok(false)
	}
}

/// A complete PoW Algorithm that uses Sha3 hashing.
/// Needs a reference to the client so it can grab the difficulty from the runtime.
pub struct CapsuleAlgorithm<C> {
	client: Arc<C>,
}

impl<C> CapsuleAlgorithm<C> {
	pub fn new(client: Arc<C>) -> Self {
		Self { client }
	}
}

// Manually implement clone. Deriving doesn't work because
// it'll derive impl<C: Clone> Clone for CapsuleAlgorithm<C>. But C in practice isn't Clone.
impl<C> Clone for CapsuleAlgorithm<C> {
	fn clone(&self) -> Self {
		Self::new(self.client.clone())
	}
}

// Here we implement the general PowAlgorithm trait for our concrete Sha3Algorithm
impl<B: BlockT<Hash = H256>, C> PowAlgorithm<B> for CapsuleAlgorithm<C>
	where
		C: HeaderBackend<B> + AuxStore + ProvideRuntimeApi<B>,
		C::Api: DifficultyApi<B, Difficulty>,
{
	type Difficulty = Difficulty;

	fn difficulty(&self, parent: B::Hash) -> Result<Self::Difficulty, Error<B>> {
		let parent_id = BlockId::<B>::hash(parent);
		let difficulty_result = self.client
			.runtime_api()
			.difficulty(&parent_id)
			.map_err(|err| {
				sc_consensus_pow::Error::Environment(format!(
					"Fetching difficulty from runtime failed: {:?}",
					err
				))
			});
		difficulty_result
	}

	fn verify(
		&self,
		_parent: &BlockId<B>,
		pre_hash: &H256,
		_pre_digest: Option<&[u8]>,
		seal: &RawSeal,
		difficulty: Self::Difficulty,
	) -> Result<bool, Error<B>> {
		// Try to construct a seal object by decoding the raw seal given
		let seal = match Seal::decode(&mut &seal[..]) {
			Ok(seal) => seal,
			Err(_) => return Ok(false),
		};

		// TODO:// difficulty verify
		// See whether the seal's difficulty meets the difficulty requirement. If not, fail fast.
		// if !hash_meets_difficulty(&seal.difficulty, difficulty) {
		// 	return Ok(false);
		// }

		// Make sure the provided work actually comes from the correct pre_hash
		let header = Compute {
			difficulty,
			pre_hash: *pre_hash,
			nonce: seal.nonce,
		};
		let raw_key = seal.pubkey;
		let pubkey = PublicKey::<Integer>::from_raw(raw_key);
		let verifier = SolutionVerifier;
		let solutions = (Solution::<Integer>::from_u256(&seal.solutions.0),
						 Solution::<Integer>::from_u256(&seal.solutions.1));
		if verifier.verify(&solutions, &pubkey, &header) {
			return Ok(true);
		}

		Ok(true)
	}
}

fn pollard_rho<C: Clone + Hash<Integer, U256>>(pubkey: PublicKey<Integer>, compute: &mut C, seed: Integer) -> Option<Solutions<Integer>> {
	// generate initial states.
	let mut state_1 = State::<Integer>::from_pub_key(pubkey, seed);
	let mut state_2 = state_1.clone();
	let mut compute_2 = compute.clone();
	let mut i = Integer::ZERO;
	let n = state_1.solution.n.clone();
	while &i < &n {
		state_1 = state_1.transit(compute).unwrap();
		state_2 = state_2.transit(&mut compute_2).unwrap().transit(&mut compute_2).unwrap();
		if &state_1.work == &state_2.work {
			if &state_1.solution != &state_2.solution {
				return Some((state_1.solution, state_2.solution));
			}
			return None;
		}
		i += 1;
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::utils::eqs_solvers;
	use rug::Integer;

	#[test]
	fn try_pollard_rho() {
		// generate a random public key.
		let p = Integer::from(383);
		let g = Integer::from(2);
		let num = Integer::from(57);
		let res = g.pow_mod_ref(&num, &p).unwrap();
		let h = Integer::from(res);
		let pubkey = PublicKey::<Integer> { p, g, h, bit_length: 32 };
		let mut loop_count = 0;
		let limit = 10;
		let mut seed = Integer::from(1);
		let mut compute = Compute {
			difficulty: 48 as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(0i32),
		};
		loop {
			if let Some(solutions) = pollard_rho(pubkey.clone(), &mut compute,seed.clone()){
				if let Some(key) = eqs_solvers(
					&solutions.0.a,
					&solutions.0.b,
					&solutions.1.a,
					&solutions.1.b,
					&solutions.1.n){
					let validate = Integer::from(pubkey.g.pow_mod_ref(&key, &pubkey.p).unwrap());
					assert_eq!(&validate, &pubkey.h, "The found key {} is not the original key {}", key, num);
					return
				} else {
					panic!("Failed to derive private key!")
				}
			} else if loop_count < limit {
				loop_count += 1;
				seed += 1;
			} else {
				panic!("Cannot find private key!")
			}
		}
	}
}
