use rug::{integer::Order, rand::RandState, Complete, Integer};
use sp_core::{H256, U256};
use rand::{self, Rng};

/// Helper function to pollard rho algorithm, 2021/01/07 added
/// modified by yangfh2004, 2022/01/31

/// Get thread local seed (0 - 1000) for running algorithm.
pub fn get_local_seed() -> Integer {
	let mut rng = rand::thread_rng();
	let seed_number = rng.gen_range(1..=1000);
	Integer::from(seed_number)
}

pub fn gen_bigint_range(rand: &mut RandState, start: &Integer, stop: &Integer) -> Integer {
	let range = Integer::from(stop - start);
	let below = range.random_below(rand);
	start + below
}

/// Convert big integer to H256 type.
pub fn bigint_h256(int: &Integer) -> H256 {
	let mut slice: Vec<u8> = int.to_digits(Order::Lsf);
	for _ in slice.len()..32{
		slice.push(0u8);
	}
	H256::from_slice(&slice)
}

/// Convert big integer to U256 type.
pub fn bigint_u256(int: &Integer) -> U256 {
	let slice: Vec<u8> = int.to_digits(Order::Lsf);
	U256::from_little_endian(&slice)
}

/// Convert U256 to big integer.
pub fn u256_bigint(unsigned: &U256) -> Integer {
	let mut num: [u8; 32] = [0u8; 32];
	unsigned.to_little_endian(&mut num);
	Integer::from_digits(&num, Order::Lsf)
}

pub fn bigint_u128(int: &Integer) -> u128 {
	int.to_u128().unwrap_or(0u128)
}

pub fn u128_bigint(unsigned: &u128) -> Integer {
	Integer::from(unsigned.to_owned())
}

/// Derive private key from a pair of collided solutions.
#[allow(dead_code)]
pub fn eqs_solvers(
	a1: &Integer,
	b1: &Integer,
	a2: &Integer,
	b2: &Integer,
	n: &Integer,
) -> Option<Integer> {
	let r = Integer::from(b1 - b2).div_rem_euc_ref(n).complete().1;
	if r == 0 {
		None
	} else {
		match r.invert_ref(n) {
			Some(inv) => {
				let res_inv = Integer::from(inv);
				let dif = Integer::from(a2 - a1);
				Some(Integer::from(res_inv * dif).div_rem_euc_ref(n).complete().1)
			},
			None => {
				let div = r.gcd(n);
				// div is the first value of (g, x, y) as a result of gcd of r and n.
				let res_l = Integer::from(b1 - b2) / &div;
				let res_r = Integer::from(a2 - a2) / &div;
				let p1 = Integer::from(n / &div);
				match res_l.invert(&p1) {
					Ok(res_inv) =>
						Some(Integer::from(res_inv * res_r).div_rem_euc_ref(&p1).complete().1),
					Err(_) => None,
				}
			},
		}
	}
}
