use clvmr::{allocator::NodePtr, reduction::EvalErr, Allocator};

pub fn curry(
    a: &mut Allocator,
    program: NodePtr,
    curried_args: NodePtr,
) -> Result<NodePtr, EvalErr> {
    let nil = a.null();
    let op_q = a.one();
    let op_a = a.new_number(2.into())?;

    let quoted_program = a.new_pair(op_q, program)?;
    let terminated_args = a.new_pair(curried_args, nil)?;
    let program_and_args = a.new_pair(quoted_program, terminated_args)?;
    let result = a.new_pair(op_a, program_and_args)?;
    Ok(result)
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
