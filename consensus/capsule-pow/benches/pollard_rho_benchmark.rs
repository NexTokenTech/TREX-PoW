use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;
use elgamal_capsule::{
    elgamal::{
        PublicKey
    }
};
use rug::{integer::Order,Integer};
use capsule_pow::{Compute, generic::Hash, pollard_rho, SolutionVerifier};
use cp_constants::Difficulty;
use sp_core::{H256, U256};
use capsule_pow::utils::{bigint_u256};
use codec::{Decode, Encode};
use sha2::{Digest,Sha256};


/// A not-yet-computed attempt to solve the proof of work. Calling the
/// compute method will compute the hash and return the seal.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct HashCompute {
    pub difficulty: Difficulty,
    pub pre_hash: H256,
    pub nonce: U256,
}

impl Hash<Integer, U256> for HashCompute {
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

pub fn pollard_rho_benchmark<C: Clone + Hash<Integer, U256>>(
    pubkey:&PublicKey,
    compute: &mut C,
){
    let mut loop_count = 0;
    let limit = 10;
    let mut seed = Integer::from(1);
    loop {
        if let Some(solutions) = pollard_rho(pubkey.clone(), compute, seed.clone()) {
            let verifier = SolutionVerifier { pubkey:pubkey.clone() };
            if let Some(key) = verifier.key_gen(&solutions) {
                let validate = Integer::from(verifier.pubkey.g.pow_mod_ref(&key.x, &verifier.pubkey.p).unwrap());
                assert_eq!(
                    &validate, &verifier.pubkey.h,
                    "The found private key is not valid!"
                );
                return;
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
    // group.bench_function("pollard rho difficulty for 24 use blake3", |b| {
    //     let difficulty = 24u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(649487),
    //         g:Integer::from(593085),
    //         h:Integer::from(336227),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 28 use blake3", |b| {
    //     let difficulty = 28u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(34203887),
    //         g:Integer::from(15693951),
    //         h:Integer::from(16216418),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 32 use blake3", |b| {
    //     let difficulty = 32u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(904155462u32),
    //         h:Integer::from(2274348566u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 24 use sha256", |b| {
    //     let difficulty = 24u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(649487),
    //         g:Integer::from(593085),
    //         h:Integer::from(336227),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 28 use sha256", |b| {
    //     let difficulty = 28u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(34203887),
    //         g:Integer::from(15693951),
    //         h:Integer::from(16216418),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 32 use sha256", |b| {
    //     let difficulty = 32u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(904155462u32),
    //         h:Integer::from(2274348566u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 31 use blake3", |b| {
    //     let difficulty = 31u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(904155462u32),
    //         h:Integer::from(2274348566u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 31 use sha256", |b| {
    //     let difficulty = 31u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(904155462u32),
    //         h:Integer::from(2274348566u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 32 use blake3", |b| {
    //     let difficulty = 32u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(904155462u32),
    //         h:Integer::from(2274348566u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 32 use sha256", |b| {
    //     let difficulty = 32u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(904155462u32),
    //         h:Integer::from(2274348566u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 33 use blake3", |b| {
    //     let difficulty = 33u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(2274348567u32),
    //         h:Integer::from(1358393698u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 33 use sha256", |b| {
    //     let difficulty = 33u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(2274348567u32),
    //         h:Integer::from(1358393698u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 34 use blake3", |b| {
    //     let difficulty = 34u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(2274348567u32),
    //         h:Integer::from(1358393698u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 34 use sha256", |b| {
    //     let difficulty = 34u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(2718559583u32),
    //         g:Integer::from(2274348567u32),
    //         h:Integer::from(1358393698u32),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 35 use blake3", |b| {
    //     let difficulty = 35u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(37078296887u64),
    //         g:Integer::from(9829944509u64),
    //         h:Integer::from(9626060443u64),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 35 use sha256", |b| {
    //     let difficulty = 35u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(37078296887u64),
    //         g:Integer::from(9829944509u64),
    //         h:Integer::from(9626060443u64),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 36 use blake3", |b| {
    //     let difficulty = 36u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(37078296887u64),
    //         g:Integer::from(9829944509u64),
    //         h:Integer::from(9626060443u64),
    //         bit_length: difficulty
    //     };
    //     let mut compute = Compute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    // group.bench_function("pollard rho difficulty for 36 use sha256", |b| {
    //     let difficulty = 36u32;
    //     let pubkey = PublicKey{
    //         p:Integer::from(37078296887u64),
    //         g:Integer::from(9829944509u64),
    //         h:Integer::from(9626060443u64),
    //         bit_length: difficulty
    //     };
    //     let mut compute = HashCompute {
    //         difficulty: difficulty as Difficulty,
    //         pre_hash: H256::from([1u8; 32]),
    //         nonce: U256::from(0i32),
    //     };
    //     b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    // });
    group.bench_function("pollard rho difficulty for 37 use blake3", |b| {
        let difficulty = 37u32;
        let pubkey = PublicKey{
            p:Integer::from(37078296887u64),
            g:Integer::from(9829944509u64),
            h:Integer::from(9626060443u64),
            bit_length: difficulty
        };
        let mut compute = Compute {
            difficulty: difficulty as Difficulty,
            pre_hash: H256::from([1u8; 32]),
            nonce: U256::from(0i32),
        };
        b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    });
    group.bench_function("pollard rho difficulty for 37 use sha256", |b| {
        let difficulty = 37u32;
        let pubkey = PublicKey{
            p:Integer::from(37078296887u64),
            g:Integer::from(9829944509u64),
            h:Integer::from(9626060443u64),
            bit_length: difficulty
        };
        let mut compute = HashCompute {
            difficulty: difficulty as Difficulty,
            pre_hash: H256::from([1u8; 32]),
            nonce: U256::from(0i32),
        };
        b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    });
    group.bench_function("pollard rho difficulty for 38 use blake3", |b| {
        let difficulty = 38u32;
        let pubkey = PublicKey{
            p:Integer::from(37078296887u64),
            g:Integer::from(9829944509u64),
            h:Integer::from(9626060443u64),
            bit_length: difficulty
        };
        let mut compute = Compute {
            difficulty: difficulty as Difficulty,
            pre_hash: H256::from([1u8; 32]),
            nonce: U256::from(0i32),
        };
        b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    });
    group.bench_function("pollard rho difficulty for 38 use sha256", |b| {
        let difficulty = 38u32;
        let pubkey = PublicKey{
            p:Integer::from(37078296887u64),
            g:Integer::from(9829944509u64),
            h:Integer::from(9626060443u64),
            bit_length: difficulty
        };
        let mut compute = HashCompute {
            difficulty: difficulty as Difficulty,
            pre_hash: H256::from([1u8; 32]),
            nonce: U256::from(0i32),
        };
        b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    });
    group.bench_function("pollard rho difficulty for 39 use blake3", |b| {
        let difficulty = 39u32;
        let pubkey = PublicKey{
            p:Integer::from(586834115123u64),
            g:Integer::from(547735195159u64),
            h:Integer::from(420654053502u64),
            bit_length: difficulty
        };
        let mut compute = Compute {
            difficulty: difficulty as Difficulty,
            pre_hash: H256::from([1u8; 32]),
            nonce: U256::from(0i32),
        };
        b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    });
    group.bench_function("pollard rho difficulty for 39 use sha256", |b| {
        let difficulty = 39u32;
        let pubkey = PublicKey{
            p:Integer::from(586834115123u64),
            g:Integer::from(547735195159u64),
            h:Integer::from(420654053502u64),
            bit_length: difficulty
        };
        let mut compute = HashCompute {
            difficulty: difficulty as Difficulty,
            pre_hash: H256::from([1u8; 32]),
            nonce: U256::from(0i32),
        };
        b.iter(move || pollard_rho_benchmark(&pubkey,&mut compute))
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

