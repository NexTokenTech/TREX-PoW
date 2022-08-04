use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use elgamal_trex::elgamal::PublicKey;
use rand::{self, Rng};
use rug::Integer;
use sp_core::U256;
use trex_pow::{generic::Hash, algorithm::PollardRhoHash, SolutionVerifier};

/// Get thread local seed (0 - 1000) for running algorithm.
fn get_local_seed() -> Integer {
	let mut rng = rand::thread_rng();
	let seed_number = rng.gen_range(1..=1000);
	Integer::from(seed_number)
}

pub fn run_pollard_rho<C: Clone + Hash<Integer, U256>>(pubkey: &PublicKey, compute: &mut C) {
	let seed = get_local_seed();
	let puzzle = pubkey.clone();
	if let Some(solutions) = puzzle.solve(compute, seed.clone()) {
		let verifier = SolutionVerifier { pubkey: pubkey.clone() };
		if let Some(key) = verifier.key_gen(&solutions) {
			let validate = Integer::from(
				verifier.pubkey.g.pow_mod_ref(&key.x, &verifier.pubkey.p).unwrap(),
			);
			assert_eq!(&validate, &verifier.pubkey.h, "The found private key is not valid!");
			return
		} else {
			panic!("Failed to derive private key!")
		}
	} else {
		panic!("Cannot find private key!")
	}
}

pub fn run_pollard_rho_parallel<C: Clone + Hash<Integer, U256>>(pubkey: &PublicKey, compute: &mut C, flag: Arc<AtomicBool>) {
	let seed = get_local_seed();
	let puzzle = pubkey.clone();
	let grain_size = 10000;
	if let Some(solutions) = puzzle.solve_parallel(compute, seed.clone(), grain_size, flag.clone()) {
		let verifier = SolutionVerifier { pubkey: pubkey.clone() };
		if let Some(key) = verifier.key_gen(&solutions) {
			let validate = Integer::from(
				verifier.pubkey.g.pow_mod_ref(&key.x, &verifier.pubkey.p).unwrap(),
			);
			assert_eq!(&validate, &verifier.pubkey.h, "The found private key is not valid!");
			return
		} else {
			panic!("Failed to derive private key!");
		}
	} else {
		// check if the solution were found by other workers or the search is just failed.
		let found = flag.load(Ordering::Relaxed);
		if found {
			return
		}
		panic!("None of workers can find private key!");
	}
}
