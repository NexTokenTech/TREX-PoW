use trex_constants::{MAX_DIFFICULTY, MIN_DIFFICULTY};
use elgamal_capsule::{elgamal::PublicKey, generate_pub_key, utils::u256_bigint};
use rug::rand::RandState;
use sp_core::U256;

/// The raw form of integer as seeds to derive a chain of public keys.
pub type RawKeySeeds = [U256; (MAX_DIFFICULTY - MIN_DIFFICULTY) as usize];
pub type Keychain = Vec<PublicKey>;

/// Yield a list of new public keys from seeds generated from public keys in previous block.
pub fn yield_pub_keys(seeds: RawKeySeeds) -> Keychain {
	seeds.iter().enumerate().map(|(index, u_seed)| {
		let mut rand = RandState::new_mersenne_twister();
		let seed = u256_bigint(u_seed);
		let bit_length = (index + MIN_DIFFICULTY as usize) as u32;
		generate_pub_key(&mut rand, bit_length, seed)
	}).collect()
}
