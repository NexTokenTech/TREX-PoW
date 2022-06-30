use capsule_pow::{
	generic::Hash, pollard_rho, utils::bigint_u256, Compute as Blake3Compute, SolutionVerifier,
};
use codec::{Decode, Encode};
use cp_constants::Difficulty;
use criterion::{criterion_group, criterion_main, Criterion};
use elgamal_capsule::elgamal::PublicKey;
use rand::{self, Rng};
use rug::{integer::Order, Integer};
use sha2::{Digest, Sha256};
use sp_core::{H256, U256};
use std::time::Duration;

/// A not-yet-computed attempt to solve the proof of work. Calling the
/// compute method will compute the hash and return the seal.
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

pub fn pollard_rho_benchmark<C: Clone + Hash<Integer, U256>>(pubkey: &PublicKey, compute: &mut C) {
	let mut loop_count = 0;
	let limit = 10;
	// let mut rand = RandState::new_mersenne_twister();
	let mut rng = rand::thread_rng();
	let seed_number = rng.gen_range(1..=10);
	let mut seed = Integer::from(seed_number);
	// let mut seed = Integer::from(1);
	loop {
		if let Some(solutions) = pollard_rho(pubkey.clone(), compute, seed.clone()) {
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
		} else if loop_count < limit {
			loop_count += 1;
			seed += 1;
		} else {
			panic!("Cannot find private key!")
		}
	}
}

fn criterion_benchmark(c: &mut Criterion) {
	let mut group = c.benchmark_group("pollard test:");
	group
		.significance_level(0.1)
		.sample_size(10)
		.measurement_time(Duration::from_secs(110));
	group.bench_function("pollard rho difficulty for 32 use blake3", |b| {
		let difficulty = 32u32;
		let pubkey = PublicKey {
			p: Integer::from(2718559583u32),
			g: Integer::from(904155462u32),
			h: Integer::from(2274348566u32),
			bit_length: difficulty,
		};
		let mut compute = Blake3Compute {
			difficulty: difficulty as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(0i32),
		};
		b.iter(move || pollard_rho_benchmark(&pubkey, &mut compute))
	});
	group.bench_function("pollard rho difficulty for 32 use sha256", |b| {
		let difficulty = 32u32;
		let pubkey = PublicKey {
			p: Integer::from(2718559583u32),
			g: Integer::from(904155462u32),
			h: Integer::from(2274348566u32),
			bit_length: difficulty,
		};
		let mut compute = Sha256Compute {
			difficulty: difficulty as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(0i32),
		};
		b.iter(move || pollard_rho_benchmark(&pubkey, &mut compute))
	});
	group.bench_function("pollard rho difficulty for 33 use blake3", |b| {
		let difficulty = 33u32;
		let pubkey = PublicKey {
			p: Integer::from(2718559583u32),
			g: Integer::from(2274348567u32),
			h: Integer::from(1358393698u32),
			bit_length: difficulty,
		};
		let mut compute = Blake3Compute {
			difficulty: difficulty as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(0i32),
		};
		b.iter(move || pollard_rho_benchmark(&pubkey, &mut compute))
	});
	group.bench_function("pollard rho difficulty for 33 use sha256", |b| {
		let difficulty = 33u32;
		let pubkey = PublicKey {
			p: Integer::from(2718559583u32),
			g: Integer::from(2274348567u32),
			h: Integer::from(1358393698u32),
			bit_length: difficulty,
		};
		let mut compute = Sha256Compute {
			difficulty: difficulty as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(0i32),
		};
		b.iter(move || pollard_rho_benchmark(&pubkey, &mut compute))
	});
	group.bench_function("pollard rho difficulty for 34 use blake3", |b| {
		let difficulty = 34u32;
		let pubkey = PublicKey {
			p: Integer::from(2718559583u32),
			g: Integer::from(2274348567u32),
			h: Integer::from(1358393698u32),
			bit_length: difficulty,
		};
		let mut compute = Blake3Compute {
			difficulty: difficulty as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(0i32),
		};
		b.iter(move || pollard_rho_benchmark(&pubkey, &mut compute))
	});
	group.bench_function("pollard rho difficulty for 34 use sha256", |b| {
		let difficulty = 34u32;
		let pubkey = PublicKey {
			p: Integer::from(2718559583u32),
			g: Integer::from(2274348567u32),
			h: Integer::from(1358393698u32),
			bit_length: difficulty,
		};
		let mut compute = Sha256Compute {
			difficulty: difficulty as Difficulty,
			pre_hash: H256::from([1u8; 32]),
			nonce: U256::from(0i32),
		};
		b.iter(move || pollard_rho_benchmark(&pubkey, &mut compute))
	});
	group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
