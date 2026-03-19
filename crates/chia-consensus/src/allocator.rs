use clvmr::allocator::Allocator;

use crate::flags::ConsensusFlags;

/// Construct an Allocator with a heap-size limit or not, depending on the flags.
pub fn make_allocator(flags: ConsensusFlags) -> Allocator {
    if flags.contains(ConsensusFlags::LIMIT_HEAP) {
        Allocator::new_limited(500_000_000)
    } else {
        Allocator::new_limited(u32::MAX as usize)
    }
}
