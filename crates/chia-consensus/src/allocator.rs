use clvmr::allocator::Allocator;
use clvmr::chia_dialect::LIMIT_HEAP;

/// Construct an Allocator with a heap-size limit or not, depending on the flags.
pub fn make_allocator(flags: u32) -> Allocator {
    if flags & LIMIT_HEAP != 0 {
        Allocator::new_limited(500_000_000)
    } else {
        Allocator::new_limited(u32::MAX as usize)
    }
}
