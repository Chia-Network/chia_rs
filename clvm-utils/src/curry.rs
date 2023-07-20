use clvmr::{allocator::NodePtr, Allocator};

use crate::{clvm_list, clvm_quote, Result, ToClvm};

pub fn curry(a: &mut Allocator, program: NodePtr, args: NodePtr) -> Result<NodePtr> {
    clvm_list!(2, clvm_quote!(program), args).to_clvm(a)
}

#[cfg(test)]
mod tests {
    use crate::{clvm_curried_args, ToClvm};

    use super::*;

    use clvmr::{serde::node_to_bytes, Allocator};
    use hex::ToHex;

    #[test]
    fn test_curry() {
        let mut a = Allocator::new();

        let program = a.new_number(2.into()).unwrap();
        let args = clvm_curried_args!(5, 8).to_clvm(&mut a).unwrap();
        let curried = curry(&mut a, program, args).unwrap();

        let bytes = node_to_bytes(&a, curried).unwrap();

        assert_eq!(
            bytes.encode_hex::<String>(),
            "ff02ffff0102ffff04ffff0105ffff04ffff0108ff01808080"
        );
    }
}
