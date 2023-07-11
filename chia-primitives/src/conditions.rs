use clvm_utils::new_list;
use clvmr::{allocator::NodePtr, reduction::EvalErr, Allocator};

pub fn create_coin(
    a: &mut Allocator,
    puzzle_hash: &[u8; 32],
    amount: u64,
) -> Result<NodePtr, EvalErr> {
    let code = a.new_number(51.into())?;
    let puzzle_hash = a.new_atom(puzzle_hash)?;
    let amount = a.new_number(amount.into())?;
    new_list(a, &[code, puzzle_hash, amount])
}
