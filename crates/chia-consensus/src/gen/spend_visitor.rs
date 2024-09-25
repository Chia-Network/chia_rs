use crate::gen::conditions::{Condition, SpendConditions};
use clvmr::allocator::Allocator;

// These are customization points for the condition parsing and validation. The
// mempool wants to record additional information than plain consensus
// validation, so it hooks into these.
pub trait SpendVisitor {
    fn new_spend(spend: &mut SpendConditions) -> Self;
    fn condition(&mut self, spend: &mut SpendConditions, c: &Condition);
    fn post_spend(&mut self, a: &Allocator, spend: &mut SpendConditions);
}
