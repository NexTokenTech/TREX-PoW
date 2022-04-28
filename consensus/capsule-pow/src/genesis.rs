use crate::{Seal, Solution};
use elgamal_capsule::RawPublicKey;
use sp_core::U256;
use cp_constants::Difficulty;

pub fn genesis_seal(difficulty:Difficulty) -> Seal {
    let genesis_solution = Solution::<U256> {
        a: U256::from(1i32),
        b: U256::from(1i32),
        n: U256::from(1i32),
    };
    return Seal {
        difficulty,
        pubkey: RawPublicKey {
            p: U256::from(1i32),
            g: U256::from(1i32),
            h: U256::from(1i32),
            bit_length: difficulty as u32,
        },
        solutions: (genesis_solution.clone(), genesis_solution),
        nonce: U256::from(1i32),
    }
}