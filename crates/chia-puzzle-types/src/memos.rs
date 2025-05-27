use clvm_traits::{FromClvm, ToClvm};
use clvmr::NodePtr;

/// The purpose of this type is to be an optional field at the end of a create coin condition
/// or payment in the notarized payment list. It can either be nil (no memos specified) or an
/// extra field that is typically a list of memos (although can technically be any structure).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(untagged, list)]
pub enum Memos<T = NodePtr> {
    /// No memos specified
    #[default]
    None,
    /// An arbitrary CLVM structure that represents the memos
    Some(T),
}
