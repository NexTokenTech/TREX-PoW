mod hash;
mod runner;

use criterion::{criterion_group, criterion_main, Criterion};
use elgamal_trex::{elgamal::PublicKey, KeyGenerator, RawKey};
use hash::Sha256Compute;
use rug::{rand::RandState, Integer};
use runner::{run_pollard_rho, run_pollard_rho_distributed, run_pollard_rho_parallel};
use sp_core::{H256, U256};
use std::{
	sync::{atomic::AtomicBool, Arc},
	thread,
	time::Duration,
};
use trex_constants::Difficulty;
use trex_pow::hash::Blake3Compute;

// Number of CPU cores in distributed benchmarking.
const N_CPU: u8 = 4;

/// helper function to get a preset pubkey.
fn get_preset_pubkey(diff: u32) -> PublicKey {
	// generate a random public key.
	let p = Integer::from(1);
	let g = Integer::from(1);
	let h = Integer::from(1);
	let old_pubkey = PublicKey { p, g, h, bit_length: diff };
	let mut rand = RandState::new_mersenne_twister();
	let raw_pubkey = old_pubkey.to_raw().yield_pubkey(&mut rand, diff);
	PublicKey::from_raw(raw_pubkey)
}

/// helper function to get a dummy data block for sha256 hashing.
fn get_sha256_block(diff: u32) -> Sha256Compute {
	Sha256Compute {
		difficulty: diff as Difficulty,
		pre_hash: H256::from([1u8; 32]),
		nonce: U256::from(0i32),
	}
}

/// helper function to get a dummy data block for blake3 hashing.
fn get_blake3_block(diff: u32) -> Blake3Compute {
	Blake3Compute {
		difficulty: diff as Difficulty,
		pre_hash: H256::from([1u8; 32]),
		nonce: U256::from(0i32),
	}
}

/// Use a multi-thread parallel computing to run the pollard rho algorithm.
fn pollard_rho_parallel_bench(c: &mut Criterion) {
	let mut group = c.benchmark_group("pollard_rho_parallel");
	group
		.significance_level(0.1)
		.sample_size(10)
		.measurement_time(Duration::from_secs(240));

	let difficulty = 38u32;

	group.bench_function("pollard_rho_diff_38_parallel", |b| {
		let mut compute = get_blake3_block(difficulty);
		let pubkey = get_preset_pubkey(difficulty);
		b.iter(move || run_pollard_rho_parallel(&pubkey, &mut compute, N_CPU));
	});

	group.bench_function("pollard_rho_diff_38_base", |b| {
		let mut compute = get_blake3_block(difficulty);
		let pubkey = get_preset_pubkey(difficulty);
		b.iter(move || run_pollard_rho(&pubkey, &mut compute))
	});

	group.finish();
}

/// Use a multi-thread distributed computing to run the pollard rho algorithm.
fn pollard_rho_distributed_bench(c: &mut Criterion) {
	let mut group = c.benchmark_group("pollard_rho_distributed");
	group
		.significance_level(0.1)
		.sample_size(10)
		.measurement_time(Duration::from_secs(240));

	let difficulty = 38u32;

	group.bench_function("pollard_rho_diff_38_distributed", |b| {
		let pubkey = get_preset_pubkey(difficulty);
		// use 4 cores in the distributed computing
		b.iter(move || {
			let found = Arc::new(AtomicBool::new(false));
			let mut threads = Vec::new();
			for _ in 0..N_CPU {
				let flag = found.clone();
				let pubkey = pubkey.clone();
				let mut compute = get_blake3_block(difficulty);
				threads.push(thread::spawn(move || {
					run_pollard_rho_distributed(&pubkey, &mut compute, flag);
				}))
			}
			threads.into_iter().for_each(|thread| {
				thread.join().expect("The thread creating or execution failed !")
			});
		})
	});

	group.bench_function("pollard_rho_diff_38_base", |b| {
		let mut compute = get_blake3_block(difficulty);
		let pubkey = get_preset_pubkey(difficulty);
		b.iter(move || run_pollard_rho(&pubkey, &mut compute))
	});

	group.finish();
}

fn pollard_rho_hash_bench(c: &mut Criterion) {
	let mut group = c.benchmark_group("pollard_rho_hash");
	group
		.significance_level(0.1)
		.sample_size(10)
		.measurement_time(Duration::from_secs(120));

	for i in 34..37 {
		let func_id = format!("pollard_rho_diff_{i}_blake3");
		group.bench_function(func_id, |b| {
			let difficulty = i;
			let mut compute = get_blake3_block(difficulty);
			let pubkey = get_preset_pubkey(difficulty);
			b.iter(move || run_pollard_rho(&pubkey, &mut compute))
		});
	}

	for i in 34..37 {
		let func_id = format!("pollard_rho_diff_{i}_sha256");
		group.bench_function(func_id, |b| {
			let difficulty = i;
			let mut compute = get_sha256_block(difficulty);
			let pubkey = get_preset_pubkey(difficulty);
			b.iter(move || run_pollard_rho(&pubkey, &mut compute))
		});
	}

	group.finish();
}

criterion_group!(
	benches,
	pollard_rho_hash_bench,
	pollard_rho_distributed_bench,
	pollard_rho_parallel_bench
);
criterion_main!(benches);
