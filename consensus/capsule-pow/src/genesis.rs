use crate::{keychain::RawKeySeeds, Seal, Solution};
use cp_constants::{Difficulty, MAX_DIFFICULTY, MIN_DIFFICULTY};
use elgamal_capsule::RawPublicKey;
use sp_core::U256;

pub fn genesis_seal(difficulty: Difficulty) -> Seal {
	let genesis_solution =
		Solution::<U256> { a: U256::from(1i32), b: U256::from(1i32), n: U256::from(1i32) };
	let genesis_key_seeds: RawKeySeeds =
		[U256::from(1i32); (MAX_DIFFICULTY - MIN_DIFFICULTY) as usize];
	Seal {
		difficulty,
		pubkey: RawPublicKey {
			p: U256::from(1i32),
			g: U256::from(1i32),
			h: U256::from(1i32),
			bit_length: difficulty as u32,
		},
		seeds: genesis_key_seeds,
		solutions: (genesis_solution.clone(), genesis_solution),
		nonce: U256::from(1i32),
	}
}
