# Pollard Rho Mining Performance Benchmark

## Comparison between Blake3 and SHA2 hash function
Here are the statistics of blake3 and sha256 with CPU mining difficulty from 32 bit-length to 34 bit-length.
The benchmark is running on an Intel 11 gen i9 processor with macOS.

```sh
Benchmarking pollard test:/pollard rho difficulty for 32 use blake3: Collecting 10 samples in est                                                                                                 pollard test:/pollard rho difficulty for 32 use blake3                        
time:   [216.06 ms 218.59 ms 220.51 ms]

Benchmarking pollard test:/pollard rho difficulty for 32 use sha256: Collecting 10 samples in est                                                                                                 pollard test:/pollard rho difficulty for 32 use sha256                        
time:   [334.20 ms 346.49 ms 367.76 ms]

Benchmarking pollard test:/pollard rho difficulty for 33 use blake3: Collecting 10 samples in est                                                                                                 pollard test:/pollard rho difficulty for 33 use blake3                        
time:   [309.94 ms 342.14 ms 380.23 ms]

Benchmarking pollard test:/pollard rho difficulty for 33 use sha256: Collecting 10 samples in est                                                                                                 pollard test:/pollard rho difficulty for 33 use sha256                        
time:   [395.40 ms 404.44 ms 411.99 ms]

Benchmarking pollard test:/pollard rho difficulty for 34 use blake3: Collecting 10 samples in est                                                                                                 pollard test:/pollard rho difficulty for 34 use blake3                        
time:   [372.96 ms 384.68 ms 398.64 ms]

Benchmarking pollard test:/pollard rho difficulty for 34 use sha256: Collecting 10 samples in est                                                                                                 pollard test:/pollard rho difficulty for 34 use sha256                        
time:   [475.34 ms 497.87 ms 526.84 ms]
```

## Boost from parallel computing
The benchmark on parallel computing shows a linear boost on number of CPU cores in use.
The benchmark was running on Intel 11 gen i7 6 cores processor with macOS (4 cores in use).

```sh
pollard_rho_parallel/pollard_rho_diff_33_base 
time:   [587.55 ms 595.61 ms 602.46 ms]
pollard_rho_parallel/pollard_rho_diff_32_parallel 
time:   [92.045 ms 97.360 ms 107.71 ms]
```