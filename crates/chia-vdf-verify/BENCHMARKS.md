# Benchmark Summary: Rust chia-vdf-verify vs C++ chiavdf

## Result

**C++ was 1.75x faster → now ~1.14x faster.** The Rust VDF verifier closes
most of the gap with C++ chiavdf (GMP-backed) through pure Rust optimizations
using crates.io malachite-nz, with no C dependencies.

## Baseline (before optimization)

| | C++ (chiavdf + GMP) | Rust (chia-vdf-verify + malachite-nz) | Ratio |
|---|---|---|---|
| Single-threaded ms/proof | 8.67 ms | 15.19 ms | **C++ 1.75x faster** |

## After optimization

### Real-world: 100 mainnet reward-chain IP proofs, single-threaded

| | C++ (ms/proof) | Rust (ms/proof) | Ratio |
|---|---|---|---|
| Single-threaded | 42.85 | 48.97 | **C++ 1.14x faster** |
| Multi-threaded (32T) | 441 proofs/s | 438 proofs/s | ~parity |

100 proofs, 14 witness types (0–19), iterations 35K–78M (median ~10.6M).
Built with generic x86-64 target (no `target-cpu=native`), matching what
ships in wheels. Rust's better GIL scaling nearly closes the gap under
concurrent load.

### Core operation: nudupl + reduce (1024-bit discriminant)

| Version | Time per op | Speedup |
|---|---|---|
| Before (main branch) | 51 µs | — |
| After (malachite branch) | 19 µs | **2.7x faster** |

## Optimizations applied

| # | Optimization | Impact |
|---|---|---|
| 1 | Port from num-bigint to malachite-nz | foundation |
| 2 | Fix PyO3 bindings, release GIL | correctness + parallel |
| 3 | Discriminant bytes API (avoid repeated decimal parse) | small |
| 4 | Extract limb words without allocation in Lehmer loop | ~5% |
| 5 | Eliminate clones, use in-place negation (`NegAssign`) | ~15% |
| 6 | O(n) byte-to-integer via direct limb construction | ~30% on decompression |
| 7 | Fused multiply-accumulate (`AddMulAssign`/`SubMulAssign`) | ~10% |
| 8 | Owned-argument GCD variants, avoid double-clones | small |
| 9 | Compiler: LTO=fat, codegen-units=1 | ~5% |
| 10 | Optimize `fdiv_r`, BQFC decompression, refactor nudupl | small |
| 11 | GCD argument swap: return native Bézout cofactor | small |

## What's left

The remaining ~14% single-threaded gap comes from GMP vs malachite-nz fundamentals:

- **GMP reuses pre-allocated buffers** via thread-local scratch (`mpz_t`);
  malachite allocates fresh `Vec`s per operation.
- **GMP has hand-tuned x86-64 assembly** for core limb operations
  (`mpn_addmul_1`, `mpn_mul_basecase`); malachite uses LLVM codegen.
- **GMP's `extended_gcd` skips unused cofactors**; malachite always derives
  both Bézout coefficients (the second via a full multiply + divide).

A malachite fork adding `extended_gcd_first_cofactor` (skip second-cofactor
derivation) and `Integer × i64` (avoid wrapper allocation) would likely
close the remaining gap, bringing Rust to parity with C++.

## Environment

- 1024-bit class group discriminants, 35K–78M iterations per proof
- Rust stable, release profile with LTO (generic x86-64 target)
- malachite-nz 0.9.1 (crates.io, no fork)
- GMP: system package (used by chiavdf)
- `.cargo/config.toml` with `target-cpu=native` removed — benchmarks
  reflect what ships in pip wheels
