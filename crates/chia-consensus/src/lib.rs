#![allow(clippy::large_stack_arrays)]
#![doc = include_str!("../README.md")]

pub mod additions_and_removals;
pub mod allocator;
pub mod build_compressed_block;
mod coin_id;
mod condition_sanitizers;
pub mod conditions;
pub mod consensus_constants;
pub mod error;
pub mod fast_forward;
pub mod flags;
pub mod generator_rom;
pub mod get_puzzle_and_solution;
pub mod make_aggsig_final_message;
pub mod merkle_set;
pub mod merkle_tree;
pub mod messages;
pub mod opcodes;
pub mod owned_conditions;
pub mod puzzle_fingerprint;
pub mod run_block_generator;
pub mod sanitize_int;
pub mod solution_generator;
pub mod spend_visitor;
pub mod spendbundle_conditions;
pub mod spendbundle_validation;
pub mod validation_error;

// these tests are large and expensive. They take a long time to run in
// unoptimized builds. Only run these with --release
#[cfg(not(debug_assertions))]
#[cfg(test)]
pub(crate) mod test_generators;
