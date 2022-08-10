use elgamal_trex::elgamal::PublicKey;
use rug::Integer;
use sp_core::U256;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
};
use trex_pow::{algorithm::{PollardRhoHash, get_local_seed}, generic::Hash, SolutionVerifier};

pub fn run_pollard_rho<C: Clone + Hash<Integer, U256>>(pubkey: &PublicKey, compute: &mut C) {
	let mut seed = get_local_seed();
	let puzzle = pubkey.clone();
	let mut loop_count = 0;
	let limit = 10;
	loop {
		if let Some(solutions) = puzzle.solve(compute, seed.clone()) {
			let verifier = SolutionVerifier { pubkey: pubkey.clone() };
			if let Some(key) = verifier.key_gen(&solutions) {
				let validate =
					Integer::from(verifier.pubkey.g.pow_mod_ref(&key.x, &verifier.pubkey.p).unwrap());
				assert_eq!(&validate, &verifier.pubkey.h, "The found private key is not valid!");
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

/// Test Pollard Rho with distributed distributed algorithm.
pub fn run_pollard_rho_distributed<C: Clone + Hash<Integer, U256>>(
	pubkey: &PublicKey,
	compute: &mut C,
	flag: Arc<AtomicBool>,
) {
	let seed = get_local_seed();
	let puzzle = pubkey.clone();
	let grain_size = 10000;
	if let Some(solutions) = puzzle.solve_dist(compute, seed.clone(), grain_size, flag.clone()) {
		let verifier = SolutionVerifier { pubkey: pubkey.clone() };
		if let Some(key) = verifier.key_gen(&solutions) {
			let validate =
				Integer::from(verifier.pubkey.g.pow_mod_ref(&key.x, &verifier.pubkey.p).unwrap());
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
