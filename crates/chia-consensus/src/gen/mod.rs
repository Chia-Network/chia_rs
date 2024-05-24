mod coin_id;
mod condition_sanitizers;
pub mod conditions;
pub mod flags;
pub mod get_puzzle_and_solution;
pub mod messages;
pub mod opcodes;
pub mod owned_conditions;
pub mod run_block_generator;
pub mod run_puzzle;
pub mod sanitize_int;
pub mod solution_generator;
pub mod spend_visitor;
pub mod validation_error;
pub mod condition_tools;

// these tests are large and expensive. They take a long time to run in
// unoptimized builds. Only run these with --release
#[cfg(not(debug_assertions))]
#[cfg(test)]
mod test_generators;
