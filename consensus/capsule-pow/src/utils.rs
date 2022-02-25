use rug::{rand::RandState, Integer};
use rug::integer::Order;
use sp_core::{H256, U256};

/// Helper function to pollard rho algorithm, 2021/01/07 added
/// modified by yangfh2004, 2022/01/31

pub fn gen_bigint_range(rand: &mut RandState, start: &Integer, stop: &Integer) -> Integer {
    let range = Integer::from(stop - start);
    let below = range.random_below(rand);
    start + below
}

/// Convert big integer to H256 type.
pub fn bigint_h256(int: &Integer) -> H256 {
    let slice: Vec<u8> = int.to_digits(Order::Lsf);
    H256::from_slice(&slice)
}

/// Convert big integer to U256 type.
pub fn bigint_u256(int: &Integer) -> U256 {
    let slice: Vec<u8> = int.to_digits(Order::Lsf);
    U256::from_little_endian(&slice)
}