/// Maximum number of mempool items that can be skipped (not considered) during
/// the creation of a block bundle. An item is skipped if it won't fit in the
/// block we're trying to create.
pub(crate) const MAX_SKIPPED_ITEMS: u32 = 6;

/// Typical cost of a standard XCH spend. It's used as a heuristic to help
/// determine how close to the block size limit we're willing to go.
pub(crate) const MIN_COST_THRESHOLD: u64 = 6_000_000;

/// Returned from `add_spend_bundle()`/`add_spend_bundles()`, indicating
/// whether more bundles can be added.
#[derive(PartialEq)]
pub enum BuildBlockResult {
    /// More spend bundles can be added
    KeepGoing,
    /// No more spend bundles can be added. We're too close to the limit
    Done,
}

pub(crate) fn skip_result(num_skipped: u32) -> BuildBlockResult {
    if num_skipped > MAX_SKIPPED_ITEMS {
        BuildBlockResult::Done
    } else {
        BuildBlockResult::KeepGoing
    }
}
