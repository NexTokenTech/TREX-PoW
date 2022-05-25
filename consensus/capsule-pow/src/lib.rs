pub mod generic;
pub mod genesis;
pub mod utils;
use codec::{Decode, Encode};
use cp_constants::Difficulty;
use elgamal_capsule::{
	elgamal::{RawKey, RawPublicKey, PublicKey, PrivateKey},
	KeyGenerator,
};
use rug::{integer::Order, rand::RandState, Complete, Integer};
use sc_client_api::{backend::AuxStore, blockchain::HeaderBackend};
use sc_consensus_pow::{Error, PowAlgorithm};
use sp_api::ProvideRuntimeApi;
use sp_consensus_pow::{DifficultyApi, Seal as RawSeal};
use sp_core::{H256, U256};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;
use blake3;

// local packages.
pub use crate::generic::{
	CycleFinding, Hash, MapResult, Mapping, MappingError, Solution, Solutions, State,
};
use utils::{bigint_u256, gen_bigint_range, u256_bigint};

pub mod app {
	use sp_application_crypto::{app_crypto, sr25519};
	use sp_core::crypto::KeyTypeId;

	pub const ID: KeyTypeId = KeyTypeId(*b"caps");
	app_crypto!(sr25519, ID);
}

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
	pub fn try_cpu_mining<C: Clone + Hash<Integer, U256> + OnCompute<Difficulty>>(
		&self,
		compute: &mut C,
		seed: U256,
		pre_pubkey: RawPublicKey,
	) -> Option<Self> {
		let mut rand = RandState::new_mersenne_twister();
		let seed_int = u256_bigint(&seed);
		let old_pubkey = pre_pubkey;
		let difficulty = compute.get_difficulty();
		let raw_pubkey = old_pubkey.yield_pubkey(&mut rand, difficulty as u32);
		let pubkey = PublicKey::from_raw(raw_pubkey);
		if let Some(solutions) = pollard_rho(pubkey.clone(), compute, seed_int) {
			// if find the solutions, build a new seal.
			Some(Seal {
				difficulty,
				pubkey: pubkey.to_raw(),
				solutions: (solutions.0.to_u256(), solutions.1.to_u256()),
				nonce: compute.get_nonce(),
			})
		} else {
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

pub trait OnCompute<E> {
	fn get_difficulty(&self) -> E;
}

impl OnCompute<Difficulty> for Compute {
	fn get_difficulty(&self) -> Difficulty {
		self.difficulty.clone()
	}
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
	fn from_pub_key(key: PublicKey, seed: Integer) -> Self {
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

	fn get_nonce(&self) -> U256 {
		self.nonce.clone()
	}

	fn hash_integer(&self) -> Integer {
		// digest nonce by hashing with header data.
		let data = &self.encode()[..];
		let hash = blake3::hash(&data);
		// convert hash results to integer in little endian order.
		Integer::from_digits(hash.as_bytes(), Order::Lsf)
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

/// A verifier contains methods to validate mining results.
pub struct SolutionVerifier{
	pub pubkey: PublicKey
}

impl SolutionVerifier {
	/// Derive one side of the value for the equation in the pollard rho method.
	fn derive(&self, solution: &Solution<Integer>) -> Integer {
		let g_a_p = Integer::from(self.pubkey.g.pow_mod_ref(&solution.a, &self.pubkey.p).unwrap());
		let h_b_p = Integer::from(self.pubkey.h.pow_mod_ref(&solution.b, &self.pubkey.p).unwrap());
		(g_a_p * h_b_p).div_rem_euc_ref(&self.pubkey.p).complete().1
	}

	/// Verify the validation of solutions and
	fn verify(&self, solutions: &Solutions<Integer>, header: &Compute) -> bool {
		let y_1 = self.derive(&solutions.0);
		let y_2 = self.derive(&solutions.1);
		// if solutions are valid, verify the hash of nonce.
		let hash_i = header.hash_integer().div_rem_euc(self.pubkey.p.clone()).1;
		let nonce = u256_bigint(&header.nonce);
		let state = State::<Integer>::from_pub_key(self.pubkey.clone(), Integer::from(1));
		let work = state.func_f(&hash_i, &nonce).unwrap();
		y_1 == y_2 && y_1 == work
	}

	pub fn key_gen(&self, solutions: &Solutions<Integer>) -> Option<PrivateKey>{
		if let Some(key) = utils::eqs_solvers(
			&solutions.0.a,
			&solutions.0.b,
			&solutions.1.a,
			&solutions.1.b,
			&solutions.1.n,
		) {
			let generator = self.pubkey.g.clone();
			let validate = Integer::from(generator.pow_mod_ref(&key, &self.pubkey.p).unwrap());
			let new_key: Integer;
			if validate != self.pubkey.h {
				new_key = key + &solutions.1.n;
			} else {
				new_key = key;
			}
			Some(PrivateKey{
				p: self.pubkey.p.clone(),
				g: self.pubkey.g.clone(),
				x: new_key,
				bit_length: self.pubkey.bit_length.clone()
			})
		} else {
			None
		}
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
		let header = Compute { difficulty, pre_hash: *pre_hash, nonce: seal.nonce };
		let raw_key = seal.pubkey;
		let pubkey = PublicKey::from_raw(raw_key);
		let verifier = SolutionVerifier {pubkey};
		let solutions = (
			Solution::<Integer>::from_u256(&seal.solutions.0),
			Solution::<Integer>::from_u256(&seal.solutions.1),
		);
		if verifier.verify(&solutions,  &header) {
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
		let difficulty_result = self.client.runtime_api().difficulty(&parent_id).map_err(|err| {
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
		let header = Compute { difficulty, pre_hash: *pre_hash, nonce: seal.nonce };
		let raw_key = seal.pubkey;
		let pubkey = PublicKey::from_raw(raw_key);
		let verifier = SolutionVerifier {pubkey};
		let solutions = (
			Solution::<Integer>::from_u256(&seal.solutions.0),
			Solution::<Integer>::from_u256(&seal.solutions.1),
		);
		if verifier.verify(&solutions, &header) {
			return Ok(true);
		}

		Ok(true)
	}
}

pub fn pollard_rho<C: Clone + Hash<Integer, U256>>(
	pubkey: PublicKey,
	compute: &mut C,
	seed: Integer,
) -> Option<Solutions<Integer>> {
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
	fn try_pollard_rho_with_key_gen() {
		// generate a random public key.
		let difficulty = 24u32;
		let p = Integer::from(1);
		let g = Integer::from(1);
		let h = Integer::from(1);
		let old_pubkey = PublicKey { p, g, h, bit_length: difficulty };
		let mut rand = RandState::new_mersenne_twister();
		let raw_pubkey = old_pubkey.to_raw().yield_pubkey(&mut rand,difficulty);
		let pubkey = PublicKey::from_raw(raw_pubkey);
		println!("{:?}",pubkey);
		let mut loop_count = 0;
		let limit = 10;
		let mut seed = Integer::from(1);
		let mut compute = Compute {
			difficulty: difficulty as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(1i32),
		};
		loop {
			if let Some(solutions) = pollard_rho(pubkey.clone(), &mut compute, seed.clone()) {
				let verifier = SolutionVerifier { pubkey };
				if let Some(key) = verifier.key_gen(&solutions) {
					let validate = Integer::from(verifier.pubkey.g.pow_mod_ref(&key.x, &verifier.pubkey.p).unwrap());
					assert_eq!(
						&validate, &verifier.pubkey.h,
						"The found private key is not valid!"
					);
					return;
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

	#[test]
	fn gen_pub_key(){
		let difficulty = 39u32;
		let p = Integer::from(1);
		let g = Integer::from(1);
		let h = Integer::from(1);
		let old_pubkey = PublicKey { p, g, h, bit_length: difficulty };
		let mut rand = RandState::new_mersenne_twister();
		let raw_pubkey = old_pubkey.to_raw().yield_pubkey(&mut rand,difficulty);
		let pubkey = PublicKey::from_raw(raw_pubkey);
		println!("{:?}",pubkey);
	}
}
