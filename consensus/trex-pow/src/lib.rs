pub mod algorithm;
pub mod generic;
pub mod genesis;
pub mod hash;
mod keychain;
pub mod parallel_mining;
pub mod utils;

use codec::{Decode, Encode};
use elgamal_trex::{
	elgamal::{PrivateKey, PublicKey, RawKey, RawPublicKey},
	Seed,
};
use log::{info, warn};
use rug::{rand::RandState, Complete, Integer};
use sc_client_api::{backend::AuxStore, blockchain::HeaderBackend};
use sc_consensus_pow::{Error, PowAlgorithm};
use sp_api::ProvideRuntimeApi;
use sp_consensus_pow::{DifficultyApi, Seal as RawSeal};
use sp_core::{H256, U256};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;
use trex_constants::{Difficulty, INIT_DIFFICULTY, MAX_DIFFICULTY, MIN_DIFFICULTY};

// local packages.
pub use crate::generic::{
	CycleFinding, Hash, MapResult, Mapping, MappingError, Solution, Solutions, State,
};
use crate::keychain::RawKeySeedsData;
use crate::utils::bigint_u128;
use algorithm::PollardRhoHash;
pub use hash::Blake3Compute as Compute;
use keychain::{yield_pub_keys, RawKeySeeds};
use std::sync::atomic::AtomicBool;
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
	/// Mining difficulty of current block sealed by this seal.
	pub difficulty: Difficulty,
	/// The public key being mined in current block.
	pub pubkey: RawPublicKey,
	/// A list of seeds to derive the public keys for next blocks.
	pub seeds: RawKeySeeds,
	/// A pair of solution for current time-lock puzzle found by mining current block.
	pub solutions: Solutions<U256>,
	/// A nonce value to seal and verify current mining works.
	pub nonce: U256,
}

impl Seal {
	pub fn try_cpu_mining<C: Clone + Hash<Integer, U256> + OnCompute<Difficulty>>(
		&self,
		compute: &mut C,
		mining_seed: U256,
		found: Arc<AtomicBool>,
	) -> Option<Self> {
		let difficulty = compute.get_difficulty();
		let keychain = yield_pub_keys(self.seeds.clone());
		let new_pubkey = keychain[(difficulty - MIN_DIFFICULTY) as usize].clone();
		let mut new_seeds: RawKeySeeds =
			[RawKeySeedsData::U128(1u128); (MAX_DIFFICULTY - MIN_DIFFICULTY) as usize];
		for (idx, key) in keychain.into_iter().enumerate() {
			if idx < 128 {
				new_seeds[idx] = RawKeySeedsData::U128(bigint_u128(&key.yield_seed()));
			} else {
				new_seeds[idx] = RawKeySeedsData::U256(bigint_u256(&key.yield_seed()));
			}
		}
		let puzzle = new_pubkey.clone();
		if let Some(solutions) =
			puzzle.solve_parallel(compute, u256_bigint(&mining_seed), 10000, found.clone())
		{
			// if find the solutions, build a new seal.
			info!("🌩 find the solutions, build a new seal");
			Some(Seal {
				difficulty,
				pubkey: new_pubkey.to_raw(),
				seeds: new_seeds,
				solutions: (solutions.0.to_u256(), solutions.1.to_u256()),
				nonce: compute.get_nonce(),
			})
		} else {
			// found.store(false, Ordering::Relaxed);
			info!("❌ don't find the solutions, return none");
			None
		}
	}
}

/// Determine whether the given hash satisfies the given difficulty.
/// The test is done by multiplying the two together. If the product
/// overflows the bounds of U128, then the product (and thus the hash)
/// was too high.
fn hash_meets_difficulty(seal_difficulty: &Difficulty, difficulty: Difficulty) -> bool {
	seal_difficulty == &difficulty
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

/// A verifier contains methods to validate mining results.
pub struct SolutionVerifier {
	pub pubkey: PublicKey,
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
		if y_1 != y_2 {
			warn!("The solution is not valid, cannot pass the block verification!");
			return false;
		}
		// if solutions are valid, verify the hash of nonce.
		let hash_i = header.hash_integer().div_rem_euc(self.pubkey.p.clone()).1;
		let nonce = u256_bigint(&header.nonce);
		let state = State::<Integer>::from_pub_key(self.pubkey.clone(), Integer::from(1));
		let work = state.func_f(&hash_i, &nonce).unwrap();
		y_1 == work
	}

	pub fn key_gen(&self, solutions: &Solutions<Integer>) -> Option<PrivateKey> {
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
			Some(PrivateKey {
				p: self.pubkey.p.clone(),
				g: self.pubkey.g.clone(),
				x: new_key,
				bit_length: self.pubkey.bit_length.clone(),
			})
		} else {
			None
		}
	}
}

/// A minimal PoW algorithm that uses pollard rho method.
/// Difficulty is fixed at 48 bit long uint.
#[derive(Clone)]
pub struct MinTREXAlgo;

// Here we implement the minimal TREX Pow Algorithm trait
impl<B: BlockT<Hash = H256>> PowAlgorithm<B> for MinTREXAlgo {
	type Difficulty = Difficulty;

	fn difficulty(&self, _parent: B::Hash) -> Result<Self::Difficulty, Error<B>> {
		// Fixed difficulty hardcoded here
		info!("⛏ Fixed mining difficulty without adjustment: {:?}", INIT_DIFFICULTY);
		Ok(INIT_DIFFICULTY as Difficulty)
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
		let verifier = SolutionVerifier { pubkey };
		let solutions = (
			Solution::<Integer>::from_u256(&seal.solutions.0),
			Solution::<Integer>::from_u256(&seal.solutions.1),
		);
		if verifier.verify(&solutions, &header) {
			return Ok(true);
		}

		Ok(false)
	}
}

/// A complete PoW Algorithm that uses Sha3 hashing.
/// Needs a reference to the client so it can grab the difficulty from the runtime.
pub struct TREXAlgo<C> {
	client: Arc<C>,
}

impl<C> TREXAlgo<C> {
	pub fn new(client: Arc<C>) -> Self {
		Self { client }
	}
}

// Manually implement clone. Deriving doesn't work because
// it'll derive impl<C: Clone> Clone for TREXAlgorithm<C>. But C in practice isn't Clone.
impl<C> Clone for TREXAlgo<C> {
	fn clone(&self) -> Self {
		Self::new(self.client.clone())
	}
}

// Here we implement the general PowAlgorithm trait for our concrete Sha3Algorithm
impl<B: BlockT<Hash = H256>, C> PowAlgorithm<B> for TREXAlgo<C>
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
		info!("⛏ The mining difficulty is adjusted as {:?}", difficulty_result);
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

		// See whether the seal's difficulty meets the difficulty requirement. If not, fail fast.
		if !hash_meets_difficulty(&seal.difficulty, difficulty) {
			warn!("The current node difficulty cannot match the difficulty in header's seal!");
			return Ok(false);
		}

		// Make sure the provided work actually comes from the correct pre_hash
		let header = Compute { difficulty, pre_hash: *pre_hash, nonce: seal.nonce };
		let raw_key = seal.pubkey;
		let pubkey = PublicKey::from_raw(raw_key);
		let verifier = SolutionVerifier { pubkey };
		let solutions = (
			Solution::<Integer>::from_u256(&seal.solutions.0),
			Solution::<Integer>::from_u256(&seal.solutions.1),
		);
		if verifier.verify(&solutions, &header) {
			return Ok(true);
		}
		dbg!("The block header cannot be verified!");
		Ok(false)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use elgamal_trex::KeyGenerator;
	use rug::Integer;
	use std::sync::atomic::AtomicBool;
	use std::thread;

	fn get_test_pubkey(diff: u32) -> PublicKey {
		// generate a random public key.
		let p = Integer::from(1);
		let g = Integer::from(1);
		let h = Integer::from(1);
		let old_pubkey = PublicKey { p, g, h, bit_length: diff };
		let mut rand = RandState::new_mersenne_twister();
		let raw_pubkey = old_pubkey.to_raw().yield_pubkey(&mut rand, diff);
		PublicKey::from_raw(raw_pubkey)
	}

	fn get_test_header(diff: u32) -> Compute {
		Compute {
			difficulty: diff as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(1i32),
		}
	}

	#[test]
	fn try_pollard_rho_with_key_gen() {
		let difficulty = 34;
		let pubkey = get_test_pubkey(difficulty);
		let mut loop_count = 0;
		let limit = 10;
		let mut seed = Integer::from(1);
		let mut compute = get_test_header(difficulty);
		let puzzle = pubkey.clone();
		loop {
			if let Some(solutions) = puzzle.solve(&mut compute, seed.clone()) {
				let verifier = SolutionVerifier { pubkey };
				if let Some(key) = verifier.key_gen(&solutions) {
					let validate = Integer::from(
						verifier.pubkey.g.pow_mod_ref(&key.x, &verifier.pubkey.p).unwrap(),
					);
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
	fn try_pollard_rho_parallel() {
		let mut threads = Vec::new();
		let cpu_n = 4;
		let found = Arc::new(AtomicBool::new(false));
		let difficulty = 41;
		for i in 0..cpu_n {
			let flag = found.clone();
			threads.push(thread::spawn(move || {
				let seed = Integer::from(i);
				let pubkey = get_test_pubkey(difficulty);
				let puzzle = pubkey.clone();
				let mut compute = get_test_header(difficulty);
				if let Some(solutions) = puzzle.solve_parallel(&mut compute, seed, 10000, flag) {
					let verifier = SolutionVerifier { pubkey: pubkey.clone() };
					if let Some(key) = verifier.key_gen(&solutions) {
						let validate = Integer::from(
							verifier.pubkey.g.pow_mod_ref(&key.x, &verifier.pubkey.p).unwrap(),
						);
						assert_eq!(
							&validate, &verifier.pubkey.h,
							"The found private key is not valid!"
						);
						return;
					} else {
						panic!("Failed to derive private key!")
					}
				} else {
					// print!("Cannot find private key with seed {i}!");
					return;
				}
			}));
		}
		threads
			.into_iter()
			.for_each(|thread| thread.join().expect("The thread creating or execution failed !"));
	}

	#[test]
	fn gen_pub_key() {
		let difficulty = 39u32;
		let p = Integer::from(1);
		let g = Integer::from(1);
		let h = Integer::from(1);
		let old_pubkey = PublicKey { p, g, h, bit_length: difficulty };
		let mut rand = RandState::new_mersenne_twister();
		let raw_pubkey = old_pubkey.to_raw().yield_pubkey(&mut rand, difficulty);
		let pubkey = PublicKey::from_raw(raw_pubkey);
		println!("{:?}", pubkey);
	}

	#[test]
	fn test_seeds_len() {
		let mut genesis_key_seeds: RawKeySeeds =
			[RawKeySeedsData::U256(U256::from(1i32)); (MAX_DIFFICULTY - MIN_DIFFICULTY) as usize];
		for idx in 0..genesis_key_seeds.len() {
			if idx < 128 {
				genesis_key_seeds[idx] = RawKeySeedsData::U128(1u128);
			}
		}
		assert_eq!(genesis_key_seeds.encode().len(), 4288, "");

		let genesis_key_seeds_u256: RawKeySeeds =
			[RawKeySeedsData::U256(U256::from(1i32)); (MAX_DIFFICULTY - MIN_DIFFICULTY) as usize];
		assert_eq!(genesis_key_seeds_u256.encode().len(), 6336, "");

		let genesis_key_seeds_u128: RawKeySeeds =
			[RawKeySeedsData::U128(1u128); (MAX_DIFFICULTY - MIN_DIFFICULTY) as usize];
		assert_eq!(genesis_key_seeds_u128.encode().len(), 3264, "");
	}
}
