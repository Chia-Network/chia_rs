pub mod visit_spends;

pub use visit_spends::*;

/// Stack size for worker threads created by chia_rs tools.
pub const THREAD_STACK_SIZE: usize = 8 * 1024 * 1024;
