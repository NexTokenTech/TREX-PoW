use crate::generic::{
	CycleFinding, Hash, MapResult, Mapping, MappingError, Solution, Solutions, State,
};
use elgamal_trex::elgamal::PublicKey;
use rug::{Complete, Integer};
use sp_core::U256;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
};

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
	/// Floyd's cycle finding algorithms.
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

pub trait PollardRhoHash {
	/// This method solve the puzzle with pollard rho method in single thread.
	fn solve<C: Clone + Hash<Integer, U256>>(
		&self,
		compute: &mut C,
		seed: Integer,
	) -> Option<Solutions<Integer>>;
	/// This method solve the puzzle with parallel computing.
	fn solve_parallel<C: Clone + Hash<Integer, U256>>(
		&self,
		compute: &mut C,
		seed: Integer,
		grain_size: u32,
		flag: Arc<AtomicBool>,
	) -> Option<Solutions<Integer>>;
}

impl PollardRhoHash for PublicKey {
	fn solve<C: Clone + Hash<Integer, U256>>(
		&self,
		compute: &mut C,
		seed: Integer,
	) -> Option<Solutions<Integer>> {
		let mut state_1 = State::<Integer>::from_pub_key(self.clone(), seed);
		let mut state_2 = state_1.clone();
		let mut compute_2 = compute.clone();
		let mut i = Integer::ZERO;
		let n = state_1.solution.n.clone();
		while &i < &n {
			state_1 = state_1.transit(compute).unwrap();
			state_2 = state_2.transit(&mut compute_2).unwrap().transit(&mut compute_2).unwrap();
			if &state_1.work == &state_2.work {
				if &state_1.solution != &state_2.solution {
					return Some((state_1.solution, state_2.solution))
				}
				return None
			}
			i += 1;
		}
		None
	}
	fn solve_parallel<C: Clone + Hash<Integer, U256>>(
		&self,
		compute: &mut C,
		seed: Integer,
		grain_size: u32,
		flag: Arc<AtomicBool>,
	) -> Option<Solutions<Integer>> {
		// generate initial states.
		let mut state_1 = State::<Integer>::from_pub_key(self.clone(), seed);
		let mut state_2 = state_1.clone();
		let mut compute_2 = compute.clone();
		let mut i = Integer::ZERO;
		// counter to check the status of other workers
		let mut counter = 0;
		let n = state_1.solution.n.clone();
		while &i < &n {
			state_1 = state_1.transit(compute).unwrap();
			state_2 = state_2.transit(&mut compute_2).unwrap().transit(&mut compute_2).unwrap();
			// check if need to check status of other workers
			if counter >= grain_size {
				let found = flag.load(Ordering::Relaxed);
				if found {
					// if other work found the solution, drop current work.
					return None
				}
				counter = 0;
			}
			// check if found the correct solution
			if &state_1.work == &state_2.work {
				// poll found status, if found, cancel and return none.
				let found = flag.load(Ordering::Relaxed);
				if found {
					return None
				}
				if &state_1.solution != &state_2.solution {
					// found the correct solution, notify other workers.
					flag.store(true, Ordering::Relaxed);
					return Some((state_1.solution, state_2.solution))
				}
				return None
			}
			i += 1;
			counter += 1;
		}
		None
	}
}
