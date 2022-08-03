use sha2::{Digest, Sha256};
use rug::{integer::Order, Integer};
use codec::{Decode, Encode};
use sp_core::{H256, U256};
use trex_constants::Difficulty;
use trex_pow::utils::bigint_u256;
use trex_pow::generic::Hash;

/// A not-yet-computed attempt to solve the proof of work. Calling the
/// compute method will compute the SHA256 hash and return the seal.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Sha256Compute {
    pub difficulty: Difficulty,
    pub pre_hash: H256,
    pub nonce: U256,
}

impl Hash<Integer, U256> for Sha256Compute {
    fn set_nonce(&mut self, int: &Integer) {
        self.nonce = bigint_u256(int);
    }

    fn get_nonce(&self) -> U256 {
        self.nonce.clone()
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
