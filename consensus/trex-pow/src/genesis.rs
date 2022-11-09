use crate::{keychain::RawKeySeeds, Seal, Solution};
use trex_constants::{Difficulty, MAX_DIFFICULTY, MIN_DIFFICULTY};
use elgamal_trex::RawPublicKey;
use sp_core::U256;
use crate::keychain::RawKeySeedsData;

pub fn genesis_seal(difficulty: Difficulty) -> Seal {
	let genesis_solution =
		Solution::<U256> { a: U256::from(1i32), b: U256::from(1i32), n: U256::from(1i32) };
	let mut genesis_key_seeds: RawKeySeeds =
		[RawKeySeedsData::U128(1u128); (MAX_DIFFICULTY - MIN_DIFFICULTY) as usize];
	for idx in 0..genesis_key_seeds.len() {
		if idx >= (128 - MIN_DIFFICULTY) as usize {
			genesis_key_seeds[idx] = RawKeySeedsData::U256(U256::from(1i32));
		}
	}
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
