# Pollard Rho Mining Performance Benchmark

## Comparison between Blake3 and SHA2 hash function
Here are the statistics of blake3 and sha256 with CPU mining difficulty from 32 bit-length to 34 bit-length.
The benchmark is running on an Intel 11 gen i9 processor with macOS.

```sh
pollard_rho_hash/pollard_rho_diff_32_blake3     time:   [409.50 ms 420.96 ms 436.06 ms]
pollard_rho_hash/pollard_rho_diff_32_sha256     time:   [493.55 ms 527.72 ms 559.01 ms]
pollard_rho_hash/pollard_rho_diff_33_blake3     time:   [512.26 ms 586.64 ms 678.48 ms]
pollard_rho_hash/pollard_rho_diff_33_sha256     time:   [702.96 ms 776.87 ms 842.88 ms]
pollard_rho_hash/pollard_rho_diff_34_blake3     time:   [670.50 ms 719.43 ms 783.99 ms]
pollard_rho_hash/pollard_rho_diff_34_sha256     time:   [999.77 ms 1.1006 s 1.2343 s]


```

## Boost from distributed computing
The benchmark on distributed computing shows a boost with square root of the number of CPU cores in use.
The benchmark was running on Intel 11 gen i7 6 cores processor with macOS (2-4 cores in use, 
theoretically 2X speed, actually 1.56X speed).
The mining difficulty for benchmarking is 38 bit-length.

```sh
pollard_rho_distributed/pollard_rho_diff_38_base        1 CPUs    time:   [3.2874 s 3.6982 s 3.9335 s]
pollard_rho_distributed/pollard_rho_diff_38_distributed 2 CPUs    time:   [2.4716 s 2.5862 s 2.6777 s]
pollard_rho_distributed/pollard_rho_diff_38_distributed 3 CPUs    time:   [2.3070 s 2.4986 s 2.6445 s]
pollard_rho_distributed/pollard_rho_diff_38_distributed 4 CPUs    time:   [2.0685 s 2.2158 s 2.3725 s]
```