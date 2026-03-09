use clvmr::allocator::Allocator;

use crate::flags::ConsensusFlags;

/// Construct an Allocator with a heap-size limit or not, depending on the flags.
pub fn make_allocator(flags: ConsensusFlags) -> Allocator {
    Allocator::new_limited(heap_limit_for_flags(flags))
}

/// Get the heap limit size based on consensus flags.
pub fn heap_limit_for_flags(flags: ConsensusFlags) -> usize {
    if flags.contains(ConsensusFlags::LIMIT_HEAP) {
        500_000_000
    } else {
        u32::MAX as usize
    }
}
