# Subagent: LIMIT_HEAP cleanup in run_block_generator2

## Background

`run_block_generator2` in chia_rs is a consensus code path. It currently accepts a `flags` argument
that includes `LIMIT_HEAP` — a mempool-policy flag that caps allocator size. The concern is that
mempool policy flags should not leak into consensus code paths.

Arvid confirmed (2026-03-11 Keybase) that mempool no longer uses `run_block_generator2`, so
`LIMIT_HEAP` support there is dead weight that muddies the separation of concerns.

Richard's proposal (Keybase 11:49 today):
> "let me draw up a PR where `run_block_generator2` just plain ignores/returns error if `LIMIT_HEAP` is set"

## Task

### Part 1: chia_rs PR

Working directory: `~/projects/chia_rs/limit-heap-error/` (branch `limit-heap-error`)
Push to: `chia` remote (Chia-Network/chia_rs) — this is Richard's repo, NOT a GitHub fork,
so push directly to the `chia` remote, not `origin`.

1. Find where `LIMIT_HEAP` / `LIMIT_HEAP_SIZE` is defined and used in `run_block_generator2`.
   Key files to look at:
   - `crates/chia-consensus/src/gen/run_block_generator.rs`
   - `crates/chia-consensus/src/flags.rs` (or similar)
   
2. Make `run_block_generator2` return an `Err` immediately if `LIMIT_HEAP` is set in flags.
   Something like:
   ```rust
   if flags & LIMIT_HEAP != 0 {
       return Err(Error::GeneratorRuntimeError); // or appropriate error variant
   }
   ```
   Use whatever error type/variant is already used for invalid inputs.

3. Update any tests that pass `LIMIT_HEAP` to `run_block_generator2` — they should either:
   - No longer pass `LIMIT_HEAP`, or
   - Be updated to expect an error

4. Run `cargo test` to confirm all tests pass.

5. Push branch to `chia` remote: `git push chia limit-heap-error`

6. Create PR against Chia-Network/chia_rs main:
   `gh pr create --repo Chia-Network/chia_rs --head limit-heap-error --title "[LIMIT_HEAP] Return error if LIMIT_HEAP passed to run_block_generator2" --body "..."`
   
   PR body should reference the Keybase discussion: LIMIT_HEAP is a mempool-only flag and
   mempool no longer uses run_block_generator2, so this cleans up the separation of concerns.

### Part 2: chia-blockchain test branch

Working directory: `~/projects/chia-blockchain/limit-heap-error/` (branch `limit-heap-error`)

After pushing the chia_rs branch:

1. Get the git commit SHA of the pushed branch:
   `git -C ~/projects/chia_rs/limit-heap-error rev-parse HEAD`

2. Update `Cargo.toml` (and `Cargo.lock`) in the chia-blockchain worktree to use that commit:
   Find the `chia_rs` git dependency and add/update `rev = "<sha>"`.
   Also update `pyproject.toml` if needed.

3. Run `maturin develop` or equivalent to rebuild with the new chia_rs.

4. Run the chia-blockchain test suite — focus on tests that exercise `run_block_generator2`:
   ```
   python -m pytest tests/consensus/ -x -q 2>&1 | tail -30
   ```
   Or whatever test command is available. Check `Makefile` first.

5. Report whether tests pass, and note any that fail due to the LIMIT_HEAP change.

## Notes

- Profile: `origin` = richardkiss personal fork, `chia` = Chia-Network corporate remote
- Richard has merge access to chia_rs and clvm_rs
- Do NOT push to chia-blockchain corporate remote — that needs a PR via fork
- The chia-blockchain branch is just for local testing, no PR needed yet

## Output

Write results to `~/DAIMON/inbox/limit-heap-error.md`:
- What changes were made and why
- PR link for chia_rs
- Test results from chia-blockchain
- Any issues or surprises found
