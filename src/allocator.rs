use clvmr::allocator::Allocator;
use clvmr::chia_dialect::LIMIT_HEAP;

pub fn make_allocator(flags: u32) -> Allocator {
    if flags & LIMIT_HEAP != 0 {
        Allocator::new_limited(500000000, 62500000, 62500000)
    } else {
        Allocator::new()
    }
}
