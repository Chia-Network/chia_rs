# AGENTS.md

## Cursor Cloud specific instructions

This is a Rust workspace monorepo for the Chia blockchain (`chia_rs`), with Python bindings (PyO3/maturin) and WASM bindings. It is a library codebase with no long-running services.

### System prerequisites

The following system packages must be installed (beyond what the update script covers):
- `python3.12-venv`, `python3-dev` — needed to create the venv for PyO3 linking
- `libssl-dev` — needed for `cargo clippy --all-features` (the `openssl` feature)
- `g++` / `libstdc++-13-dev` — C++ compiler and stdlib (used by `chia-pos2` and fuzz crates); if `c++` symlink points to clang, you may need `sudo update-alternatives --set c++ /usr/bin/g++` and `sudo ln -s /usr/lib/gcc/x86_64-linux-gnu/13/libstdc++.so /usr/lib/x86_64-linux-gnu/libstdc++.so`

### Development workflow

All standard commands are documented in the README. Key commands:

| Task | Command |
|---|---|
| Build | `cargo build --workspace` |
| Lint (fmt) | `cargo fmt --all -- --check` |
| Lint (clippy) | `cargo clippy --workspace --all-features --all-targets` |
| Lint (prettier) | `npx prettier --check .` |
| Rust tests | `cargo test --workspace` |
| Python wheel | `. ./venv/bin/activate && maturin develop -m wheel/Cargo.toml` |
| Python tests | `. ./venv/bin/activate && pytest tests` |

### Gotchas

- **Python venv must be active** for `cargo build`/`cargo test` of the full workspace (PyO3 in the `wheel` crate requires linking against Python).
- **Python tests are slow** (~12 min in debug mode with 4 xdist workers). The `test_additions_and_removals` and `test_block_builder` tests dominate runtime.
- `cargo clippy --all-features` triggers the `openssl` feature, requiring `libssl-dev`.
- The `c++` system symlink defaults to clang++ in the VM image, but `chia-pos2` and fuzz crates need `libstdc++` headers/libs accessible; switching `c++` to `g++` and symlinking `libstdc++.so` resolves this.
- npm is only used for Prettier formatting; no Node.js runtime services exist.
