# Pollard Rho Mining Performance Benchmark

## Comparison between Blake3 and SHA2 hash function
Here are the statistics of blake3 and sha256 with CPU mining difficulty from 32 bit-length to 34 bit-length.
The benchmark is running on an Intel 11 gen i9 processor with macOS.

```sh
pollard_rho_hash/pollard_rho_diff_32_blake3     time:   [248.44 ms 257.30 ms 263.45 ms]
pollard_rho_hash/pollard_rho_diff_32_sha256     time:   [588.25 ms 601.73 ms 609.61 ms]
pollard_rho_hash/pollard_rho_diff_33_blake3     time:   [436.24 ms 452.27 ms 462.17 ms]
pollard_rho_hash/pollard_rho_diff_33_sha256     time:   [500.00 ms 517.21 ms 535.64 ms]
pollard_rho_hash/pollard_rho_diff_34_blake3     time:   [1.3144 s 1.4603 s 1.5826 s]
pollard_rho_hash/pollard_rho_diff_34_sha256     time:   [1.6991 s 1.9128 s 2.1527 s]


```

## Boost from distributed computing
The benchmark on distributed computing shows a boost with square root of the number of CPU cores in use.
The benchmark was running on Intel 11 gen i7 6 cores processor with macOS (4 cores in use, 
theoretically 2X speed, actually 1.56X speed).
The mining difficulty for benchmarking is 38 bit-length.

```sh
pollard_rho_distributed/pollard_rho_diff_38_base        1 CPUs    time:   [1.8239 s 2.0399 s 2.2700 s]
pollard_rho_distributed/pollard_rho_diff_38_distributed 4 CPUs    time:   [1.1468 s 1.2208 s 1.3376 s]
```