![Build](https://github.com/Chia-Network/chia_rs/actions/workflows/build-crate-and-npm.yml/badge.svg)
![Test](https://github.com/Chia-Network/chia_rs/actions/workflows/build-test.yml/badge.svg)
![PyPI](https://img.shields.io/pypi/v/chia_rs?logo=pypi)
![PyPI - Format](https://img.shields.io/pypi/format/chia_rs?logo=pypi)
![GitHub](https://img.shields.io/github/license/Chia-Network/chia_rs?logo=Github)
[![Coverage Status](https://coveralls.io/repos/github/Chia-Network/chia_rs/badge.svg?branch=main)](https://coveralls.io/github/Chia-Network/chia_rs?branch=main)

This cargo workspace contains code useful for working with the Chia network.

# Tests

To run tests:

```
cargo test --all
```

Some slow tests are only enabled in optimized builds, so it may also be a good
idea to run the tests in release mode:

```
cargo test --all --release
```

You may need a python virtual environment activated for the tests to link.
This seems to be caused by the pyo3 dependency in the `wheel`.

# Benchmarks

To run benchmarks for a specific crate:

```
cargo bench -- --save-baseline before
<make change>
cargo bench -- --save-baseline after
critcmp after before
```

You can also run all the benchmarks by including `--workspace` on the command
line.

# pre-commit

This repository has a pre-commit configuration, which is hooked into git by
running:

```
pre-commit install --hook-type pre-commit --hook-type pre-push
```

It runs `cargo fmt` on all crates on every commit, and runs clippy and builds on
push.

To run all checks explicitly (without pushing), run:

```
pre-commit run --all --hook-stage pre-push
```

# python bindings

The `wheel` crate is a single python wheel that exports the functionality of
all crates in the repository.

It's built with `maturin`. You need to have activated a python virtual
environment for the build to work.

```
pip install maturin
cd wheel
maturin develop
```

Once built, the python tests can be run, from the root of the repository. Note
that the tests require that `chia-blockchain` and `blspy` wheels are installed.

```
pytest tests
```

# Fuzzers

Fuzzers can't be run or listed for the whole workspace, but only for individual
crates. There is a tool to generate a fuzzing corpus from a blockchain database.
It's run like this:

```
cd crates/chia-tools
cargo run --release --bin gen-corpus -- --help
```

The following crates have fuzzers:

* chia-bls
* chia-protocol
* chia-puzzles
* clvm-utils
* chia (the root crate)

To list and run fuzzers:

```
cargo fuzz list
```

```
cargo fuzz run <name-of-fuzzer>
```

# Bumping version number

Make sure you have `cargo-workspaces` installed:

```
cargo install cargo-workspaces
```

To bump the versions of all relevant crates:

```
cargo ws version --all --no-git-commit
```

Select "minor update" if there has not been any incompatible API changes,
otherwise "major update".

# Running in Docker
Start basic local image in docker as a detached service
```
docker-compose up -d
```
Run tests command in container
```
docker exec chia_rs cargo test --all
```
Start interactive bash session. You can then run all commands above as you would on host.
```
docker exec -it chia_rs bash
```
