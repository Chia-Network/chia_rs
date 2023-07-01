use clvmr::{allocator::NodePtr, reduction::EvalErr, Allocator};

pub fn curry(
    allocator: &mut Allocator,
    program: NodePtr,
    args: &[NodePtr],
) -> Result<NodePtr, EvalErr> {
    let nil = allocator.null();
    let op_q = allocator.one();
    let op_a = allocator.new_number(2.into())?;
    let op_c = allocator.new_number(4.into())?;

    let quoted_program = allocator.new_pair(op_q, program)?;
    let mut quoted_args = allocator.one();

    for arg in args.iter().rev() {
        let quoted_arg = allocator.new_pair(op_q, *arg)?;
        let terminated_args = allocator.new_pair(quoted_args, nil)?;
        let terminated_args = allocator.new_pair(quoted_arg, terminated_args)?;
        quoted_args = allocator.new_pair(op_c, terminated_args)?;
    }

    let terminated_args = allocator.new_pair(quoted_args, nil)?;
    let program_and_args = allocator.new_pair(quoted_program, terminated_args)?;
    let result = allocator.new_pair(op_a, program_and_args)?;
    Ok(result)
}

#[cfg(test)]
use hex::ToHex;

#[cfg(test)]
use clvmr::serde::node_to_bytes;

#[test]
fn test_curry() {
    let mut allocator = Allocator::new();

    let program = allocator.new_number(2.into()).unwrap();
    let arg1 = allocator.new_number(5.into()).unwrap();
    let arg2 = allocator.new_number(8.into()).unwrap();
    let curried = curry(&mut allocator, program, &[arg1, arg2]).unwrap();

    let bytes = node_to_bytes(&allocator, curried).unwrap();

    assert_eq!(
        bytes.encode_hex::<String>(),
        "ff02ffff0102ffff04ffff0105ffff04ffff0108ff01808080"
    );
}
