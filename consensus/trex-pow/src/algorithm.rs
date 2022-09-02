use std::collections::HashMap;
use crate::generic::{
	CycleFinding, Hash, MapResult, Mapping, MappingError, Solution, Solutions, State, StateHash,
};
use elgamal_trex::elgamal::PublicKey;
use rug::{Complete, Integer};
use sp_core::U256;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, RwLock, Mutex};
use std::thread;

/// This factor is to reduce the length of trails between distinguished point so that the search
/// is more efficient.
/// The expected length = sqrt(p) / 2^POINT_DST_FACTOR
const POINT_DST_FACTOR: u32 = 8;
const SEARCH_LEN_FACTOR: u32 = 8;

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
	/// This method solve the puzzle with distributed computing.
	fn solve_dist<C: Clone + Hash<Integer, U256>>(
		&self,
		compute: &mut C,
		seed: Integer,
		grain_size: u32,
		flag: Arc<AtomicBool>,
	) -> Option<Solutions<Integer>>;
	/// This method solve the puzzle with parallel computing.
	fn solve_parallel<C: Sync + Send + Clone + Hash<Integer, U256> + 'static>(
		&self,
		compute: &mut C,
		seed: Integer,
		grain_size: u32,
		flag: Arc<AtomicBool>,
		cpus: u8,
	) -> Option<Solutions<Integer>>;
	/// This method generate hash tester based on current difficulty.
	fn hash_diff(&self) -> U256;
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
		let n = Integer::from(self.p.sqrt_ref()) * SEARCH_LEN_FACTOR;
		while &i < &n {
			state_1 = state_1.transit(compute).unwrap();
			state_2 = state_2.transit(&mut compute_2).unwrap().transit(&mut compute_2).unwrap();
			if &state_1.work == &state_2.work {
				if &state_1.solution != &state_2.solution {
					// find the solution, then need to find the nonce.
					break
				}
				return None
			}
			i += 1;
		}
		// extra nonce condition against distributed computing on clusters.
		i = Integer::ZERO;
		let hash_diff = self.hash_diff();
		while &i < &n {
			// keep rolling the dices until nonce meet the condition.
			// There are difficulty / 2 zeros on the nonce.
			state_1 = state_1.transit(compute).unwrap();
			state_2 = state_2.transit(&mut compute_2).unwrap();
			let (_, overflowed_1) = state_1.hash_encode().overflowing_mul(hash_diff);
			if !overflowed_1 {
				// find the nonce with a number of leading zero bits.
				if &state_1.work == &state_2.work && &state_1.solution != &state_2.solution {
					return Some((state_1.solution, state_2.solution))
				}
				return None
			}
			i += 1;
		}
		None
	}

	fn solve_dist<C: Clone + Hash<Integer, U256>>(
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
		let n = Integer::from(self.p.sqrt_ref()) * SEARCH_LEN_FACTOR;
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
				if &state_1.solution != &state_2.solution {
					// go to next step to meet extra nonce conditions.
					break
				}
				return None
			}
			i += 1;
			counter += 1;
		}
		// extra nonce condition against distributed computing on clusters.
		i = Integer::ZERO;
		counter = 0;
		let hash_diff = self.hash_diff();
		while &i < &n {
			// keep rolling the dices until nonce meet the condition.
			// There are difficulty / 2 zeros on the nonce.
			state_1 = state_1.transit(compute).unwrap();
			state_2 = state_2.transit(&mut compute_2).unwrap();
			let (_, overflowed_1) = state_1.hash_encode().overflowing_mul(hash_diff);
			// check if need to check status of other workers
			if counter >= grain_size {
				let found = flag.load(Ordering::Relaxed);
				if found {
					// if other work found the solution, drop current work.
					return None
				}
				counter = 0;
			}
			if !overflowed_1 {
				// poll found status, if peer nodes found the result, cancel and return none.
				let found = flag.load(Ordering::Relaxed);
				if found {
					return None
				}
				// find the nonce with a number of leading zero bits.
				if &state_1.work == &state_2.work && &state_1.solution != &state_2.solution {
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

	fn solve_parallel<C: Sync + Send + Clone + Hash<Integer, U256> + 'static>(
		&self,
		compute: &mut C,
		seed: Integer,
		grain_size: u32,
		flag: Arc<AtomicBool>,
		cpus: u8,
	) -> Option<Solutions<Integer>> {
		// prepare compute arrays
		let mut threads = Vec::new();
		let collision = Arc::new(RwLock::new(HashMap::<Integer, Solution<Integer>>::new()));
		let res: Arc<Mutex<Option<Solutions<Integer>>>> = Arc::new(Mutex::new(None));
		let nonce = Arc::new(Mutex::new(Integer::from(1)));
		let max_try = 10;
		for cpu_i in 0..cpus {
			let mut new_compute = compute.clone();
			let hash_diff = self.hash_diff();
			let res_lock = res.clone();
			// shared hash map for collision detection.
			let col = collision.clone();
			let found = flag.clone();
			// the shared variable to pass final nonce value.
			let this_nonce = nonce.clone();
			let pubkey = self.clone();
			let local_seed = seed.clone() + max_try * cpu_i;
			threads.push(thread::spawn(move || {
				let mut i = Integer::ZERO;
				let mut counter = 0;
				let n = Integer::from(pubkey.p.sqrt_ref()) * SEARCH_LEN_FACTOR;
				let mut existed;
				let mut j = 0;
				let mut state = State::<Integer>::from_pub_key(pubkey.clone(), local_seed.clone());
				loop {
					while &i < &n {
						state = state.transit(&mut new_compute).unwrap();
						let (_, overflowed) = state.hash_encode().overflowing_mul(hash_diff);
						if !overflowed {
							{
								let col_map = col.read().unwrap();
								if col_map.contains_key(&state.work){
									if let Some(sol_in_map) = col_map.get(&state.work){
										if &state.solution != sol_in_map {
											// poll found status, if peer nodes found the result, cancel and return none.
											if found.load(Ordering::Relaxed) {
												return
											}
											found.store(true, Ordering::Relaxed);
											{
												// update nonce value.
												let mut nonce_guard = this_nonce.lock().unwrap();
												*nonce_guard = state.nonce;
											}
											{
												// update solution
												let mut solutions = res_lock.lock().unwrap();
												*solutions = Some((state.solution.clone(), sol_in_map.clone()));
											}
											return
										}
									} else {
										// if the two collide nodes are the same, restart the search with different seed.
										j += 1;
										state = State::<Integer>::from_pub_key(pubkey.clone(), local_seed.clone()+j);
										i = Integer::ZERO;
									}
									existed = true;
								}else{
									existed = false;
								}
							}
							if !existed {
								let mut col_map = col.write().unwrap();
								col_map.entry(state.work.clone()).or_insert(state.solution.clone());
							}
						}
						if counter >= grain_size {
							if found.load(Ordering::Relaxed) {
								// if other work found the solution, drop current work.
								return
							}
							counter = 0;
						}
						i += 1;
						counter += 1;
					}
					if j < max_try {
						// cannot find the collision in 20x length of trails between distinguished points.
						j += 1;
						state = State::<Integer>::from_pub_key(pubkey.clone(), local_seed.clone() + j);
						i = Integer::ZERO;
					} else {
						return
					}
				}
			}));
		}

		threads.into_iter().for_each(|thread|{
			thread.join().expect("The thread creating or execution failed !")});
		// update compute
		let work = Arc::try_unwrap(nonce).unwrap().into_inner().unwrap();
		compute.set_nonce(&work);
		Arc::try_unwrap(res).unwrap().into_inner().unwrap()
	}

	fn hash_diff(&self) -> U256 {
		let shift = self.bit_length / 2 - POINT_DST_FACTOR;
		if self.bit_length % 2 != 0 {
			(U256::one() << shift) + (U256::one() << (shift - 1))
		} else {
			U256::one() << shift
		}
	}
}
