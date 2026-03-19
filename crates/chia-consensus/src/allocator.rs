use clvmr::allocator::Allocator;

/// Construct an Allocator with the mempool heap limit (500 MB).
/// This is the standard allocator for all spend-bundle / mempool paths.
/// Block-validation paths create their own allocator without a low cap.
pub fn make_allocator() -> Allocator {
    Allocator::new_limited(500_000_000)
}
