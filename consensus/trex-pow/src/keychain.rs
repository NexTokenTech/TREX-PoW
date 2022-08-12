use trex_constants::{MAX_DIFFICULTY, MIN_DIFFICULTY};
use elgamal_trex::{elgamal::PublicKey, generate_pub_key, utils::u256_bigint,};
use crate::utils::u128_bigint;
use rug::Integer;
use rug::rand::RandState;
use sp_core::U256;
use codec::{Decode, Encode};

/// The raw form of integer as seeds to derive a chain of public keys.
pub type RawKeySeeds = [RawKeySeedsData; (MAX_DIFFICULTY - MIN_DIFFICULTY) as usize];

#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug, Copy)]
pub enum RawKeySeedsData{
	U128(u128),
	U256(U256)
}
pub type Keychain = Vec<PublicKey>;

/// Yield a list of new public keys from seeds generated from public keys in previous block.
pub fn yield_pub_keys(seeds: RawKeySeeds) -> Keychain {
	seeds.iter().enumerate().map(|(index, u_seed)| {
		let mut rand = RandState::new_mersenne_twister();
		let mut seed = Integer::ZERO;
		match u_seed {
			RawKeySeedsData::U128(value) => {
				seed = u128_bigint(value);
			},
			RawKeySeedsData::U256(value) =>{
				seed = u256_bigint(value);
			}
		}
		let bit_length = (index + MIN_DIFFICULTY as usize) as u32;
		generate_pub_key(&mut rand, bit_length, seed)
	}).collect()
}
