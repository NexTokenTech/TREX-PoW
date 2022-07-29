mod hash;
mod runner;

use criterion::{criterion_group, criterion_main, Criterion};
use elgamal_trex::elgamal::PublicKey;
use hash::Sha256Compute;
use rug::Integer;
use runner::{run_pollard_rho, run_pollard_rho_parallel};
use sp_core::{H256, U256};
use std::time::Duration;
use trex_constants::Difficulty;
use trex_pow::hash::Blake3Compute;
use std::sync::{Arc, atomic::AtomicBool};
use std::thread;

// Number of CPU cores in parallel benchmarking.
const N_CPU: i32 = 4;

/// helper function to get a preset pubkey.
fn get_preset_pubkey(diff: u32) -> PublicKey{
	let p = Integer::from(2718559583u32);
	let g = Integer::from(904155462u32);
	let h = Integer::from(2274348566u32);
	PublicKey {
		p,
		g,
		h,
		bit_length: diff,
	}
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

	group.bench_function("pollard_rho_diff_39_parallel", |b|{
		let difficulty = 39u32;
		let pubkey = get_preset_pubkey(difficulty);
		// use 4 cores in the parallel computing
		b.iter(move || {
			let found = Arc::new(AtomicBool::new(false));
			let mut threads = Vec::new();
			for _ in 0..N_CPU {
				let flag = found.clone();
				let pubkey = pubkey.clone();
				let mut compute = get_blake3_block(difficulty);
				threads.push(thread::spawn(move || {
					run_pollard_rho_parallel(&pubkey, &mut compute, flag);
				}))
			}
			threads.into_iter().for_each(|thread| {
				thread
					.join()
					.expect("The thread creating or execution failed !")
			});
		})
	});

	group.bench_function("pollard_rho_diff_39_base", |b| {
		let difficulty = 39u32;
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

	for i in 32..35 {
		let func_id = format!("pollard_rho_diff_{i}_blake3");
		group.bench_function(func_id, |b| {
			let difficulty = i;
			let mut compute = get_blake3_block(difficulty);
			let pubkey = get_preset_pubkey(difficulty);
			b.iter(move || run_pollard_rho(&pubkey, &mut compute))
		});
	}

	for i in 32..35 {
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

criterion_group!(benches, pollard_rho_hash_bench, pollard_rho_parallel_bench);
criterion_main!(benches);