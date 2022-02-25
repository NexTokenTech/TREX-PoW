mod generic;
mod utils;

use elgamal_wasm::generic::PublicKey;
use parity_scale_codec::{Decode, Encode};
use rug::{integer::Order, rand::RandState, Integer, Complete};
use sc_consensus_pow::{Error, PowAlgorithm};
use sha2::{Digest, Sha256};
use sp_api::ProvideRuntimeApi;
use sp_consensus_pow::{DifficultyApi, Seal as RawSeal};
use sp_core::{H256, U256};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

// local packages.
use crate::generic::{CycleFinding, DagMapping, Hash, MapResult, MappingError, Solution, State};
use utils::{bigint_u256, gen_bigint_range};

// constants.
const BIG_INT_0: Integer = Integer::ZERO;

/// A Seal struct that will be encoded to a Vec<u8> as used as the
/// `RawSeal` type.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Seal {
	pub difficulty: U256,
	pub work: H256,
	pub nonce: U256,
}

/// A not-yet-computed attempt to solve the proof of work. Calling the
/// compute method will compute the hash and return the seal.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Compute {
	difficulty: U256,
	pre_hash: H256,
	nonce: U256,
}

impl Solution<Integer> {
	pub fn new_random(n: Integer, seed: &Integer) -> Self {
		let mut rand = RandState::new_mersenne_twister();
		rand.seed(seed);
		Solution {
			a: gen_bigint_range(&mut rand, &BIG_INT_0, &n),
			b: gen_bigint_range(&mut rand, &BIG_INT_0, &n),
			n,
		}
	}
}

impl State<Integer> {
	/// Derive a new node state from a public key.
	pub fn from_pub_key(key: &PublicKey<Integer>, seed: &Integer) -> Self {
		let p_1 = Integer::from(&key.p - 1);
		let n = Integer::from( &p_1 / 2);
		let solution = Solution::new_random(n, seed);
		let g_a_p = Integer::from(key.g.pow_mod_ref(&solution.a, &key.p).unwrap());
		let h_b_p = Integer::from(key.h.pow_mod_ref(&solution.b, &key.p).unwrap());
		let y = Integer::from(g_a_p * h_b_p).div_rem_euc(key.p.clone()).1;
		// NOTE: never use 0 to initialize integers which may lead to memory corruption.
		State::<Integer> { solution, nonce: Integer::from(1), work: y }
	}
}

impl Hash<Integer> for Compute {
	fn update_nonce(&mut self, int: &Integer) {
		self.nonce = bigint_u256(int);
	}

	fn hash_integer(&self) -> Integer {
		// digest nonce by hashing with header data.
		let data = &self.encode()[..];
		let mut hasher = Sha256::new();
		hasher.update(&data);
		// convert hash results to integer in little endian order.
		Integer::from_digits(hasher.finalize().as_slice(), Order::Lsf)
	}
}

impl DagMapping<Integer> for PublicKey<Integer> {
	/// The pollard rho miner with a mapping function which is hard to compute reversely.
	fn func_f(&self, x_i: &Integer, y_i: &Integer) -> MapResult<Integer> {
		let base = &self.g;
		let h = &self.h;
		let p = &self.p;
		match x_i.mod_u(3) {
			0 => Ok(Integer::from(y_i.pow_mod_ref(x_i, p).unwrap())),
			1 => {
				let base_hash_p = Integer::from(base.pow_mod_ref(x_i, p).unwrap());
				let res = (base_hash_p * y_i).div_rem_euc_ref(p).complete();
				Ok(res.1)
			},
			2 => {
				let h_hash_p = Integer::from(h.pow_mod_ref(x_i, p).unwrap());
				Ok((h_hash_p * y_i).div_rem_euc_ref(p).complete().1)
			},
			_ => Err(MappingError),
		}
	}

	fn func_g(&self, a_i: &Integer, x_i: &Integer) -> MapResult<Integer> {
		let p_1 = Integer::from(&self.p - 1);
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
		let p_1 = Integer::from(&self.p - 1);
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

impl CycleFinding<Integer> for PublicKey<Integer> {
	/// Single Step Transition calculation between states.
	fn transit<C: Hash<Integer>>(&self, state: State<Integer>, compute: &mut C) -> MapResult<State<Integer>> {
		compute.update_nonce(&state.work);
		let raw_int = compute.hash_integer();
		let hash_i = raw_int.div_rem_euc(self.p.clone()).1;
		let work = self.func_f(&hash_i, &state.work)?;
		let a = self.func_g(&state.solution.a, &hash_i)?;
		let b = self.func_h(&state.solution.b, &hash_i)?;
		Ok(State::<Integer> {
			solution: Solution {
				a,
				b,
				n: state.solution.n,
			},
			work,
			nonce: state.work,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rug::{Complete, Integer};

	fn eqs_solvers(
		a1: &Integer,
		b1: &Integer,
		a2: &Integer,
		b2: &Integer,
		n: &Integer,
	) -> Option<Integer> {
		let r = Integer::from(b1 - b2).div_rem_euc_ref(n).complete().1;
		if r == 0 {
			None
		} else {
			match r.invert_ref(n) {
				Some(inv) => {
					let res_inv = Integer::from(inv);
					let dif = Integer::from(a2 - a1);
					Some(Integer::from(res_inv * dif).div_rem_euc_ref(n).complete().1)
				},
				None => {
					let div = r.gcd(n);
					// div is the first value of (g, x, y) as a result of gcd of r and n.
					let res_l = Integer::from(b1 - b2) / &div;
					let res_r = Integer::from(a2 - a2) / &div;
					let p1 = Integer::from(n / &div);
					match res_l.invert(&p1) {
						Ok(res_inv) =>
							Some(Integer::from(res_inv * res_r).div_rem_euc_ref(&p1).complete().1),
						Err(_) => None,
					}
				},
			}
		}
	}

	#[test]
	fn try_pollard_rho() {
		// generate a random public key.
		let p = Integer::from(383);
		let n = Integer::from(191);
		let g = Integer::from(2);
		let num = Integer::from(57);
		let res = g.pow_mod_ref(&num, &p).unwrap();
		let h = Integer::from(res);
		let pubkey = PublicKey::<Integer> { p, g, h, bit_length: 32 };
		let mut loop_count = 0;
		let mut seed = Integer::from(1);
		loop {
			// generate initial states.
			let mut state_1 = State::<Integer>::from_pub_key(&pubkey, &seed);
			let mut state_2 = state_1.clone();
			let mut compute_1 = Compute {
				difficulty: U256::from(32i32),
				pre_hash: H256::from([1u8; 32]),
				nonce: U256::from(0i32),
			};
			let mut compute_2 = compute_1.clone();
			let mut i = Integer::ZERO;
			let n = state_1.solution.n.clone();
			let limit = 10;
			while &i < &n {
				state_1 = pubkey.transit(state_1, &mut compute_1).unwrap();
				state_2 = pubkey.transit(state_2, &mut compute_2).unwrap();
				state_2 = pubkey.transit(state_2, &mut compute_2).unwrap();
				if &state_1.work == &state_2.work {
					if let Some(key) = eqs_solvers(
						&state_1.solution.a,
						&state_1.solution.b,
						&state_2.solution.a,
						&state_2.solution.b,
						&state_1.solution.n,
					){
						let res_key = Integer::from(&num.div_rem_euc_ref(&n).complete().1);
						let validate = Integer::from(pubkey.g.pow_mod_ref(&key, &pubkey.p).unwrap());
						assert_eq!(&validate, &pubkey.h, "The found key {} is not the original key {}", key, num);
						return
					}else{
						// if collision, try again.
						break
					}
				}
				i += 1;
			}
			if loop_count < limit {
				loop_count += 1;
				seed += 1;
			}else{
				break
			}
		}
		panic!("Cannot find key!")
	}
}
