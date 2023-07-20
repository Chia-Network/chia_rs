use clvmr::allocator::{Allocator, NodePtr};

use crate::{
    destructure_list, destructure_quote, match_list, match_quote, FromClvm, LazyNode, MatchByte,
    Result,
};

pub fn uncurry(a: &Allocator, node: NodePtr) -> Result<(NodePtr, NodePtr)> {
    let destructure_list!(_a, quoted_program, args) =
        <match_list!(MatchByte::<2>, match_quote!(LazyNode), LazyNode)>::from_clvm(a, node)?;
    let destructure_quote!(program) = quoted_program;
    Ok((program.0, args.0))
}
