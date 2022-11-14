use crate::gen::validation_error::{atom, check_nil, first, next, rest, ErrorCode, ValidationErr};
use ::chia_protocol::bytes::Bytes32;
use clvm_utils::tree_hash::tree_hash;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::op_utils::u64_from_bytes;
use std::convert::AsRef;

// returns parent-coin ID, amount, puzzle-reveal and solution
pub fn parse_coin_spend(
    a: &Allocator,
    coin_spend: NodePtr,
) -> Result<(&[u8], u64, NodePtr, NodePtr), ValidationErr> {
    let parent = atom(a, first(a, coin_spend)?, ErrorCode::InvalidParentId)?;
    let coin_spend = rest(a, coin_spend)?;
    let puzzle = first(a, coin_spend)?;
    let coin_spend = rest(a, coin_spend)?;
    let amount = u64_from_bytes(atom(
        a,
        first(a, coin_spend)?,
        ErrorCode::InvalidCoinAmount,
    )?);
    let coin_spend = rest(a, coin_spend)?;
    let solution = first(a, coin_spend)?;
    check_nil(a, rest(a, coin_spend)?)?;
    Ok((parent, amount, puzzle, solution))
}

pub fn get_puzzle_and_solution_for_coin(
    a: &Allocator,
    generator_result: NodePtr,
    find_parent: Bytes32,
    find_amount: u64,
    find_ph: Bytes32,
) -> Result<(NodePtr, NodePtr), ValidationErr> {
    // the output from the block generator is a list of CoinSpends
    // with (parent-coin-id puzzle-reveal amount solution)
    // this function is given the generator output and a parent_coin_id, amount
    // and puzzle_hash and it will return the puzzle and solution for that given
    // coin spend, or fail if it cannot be found
    let mut iter = first(a, generator_result)?;
    while let Some((coin_spend, next)) = next(a, iter)? {
        iter = next;
        // coin_spend is (parent puzzle amount solution)
        let (parent, amount, puzzle, solution) = parse_coin_spend(a, coin_spend)?;

        // we want to avoid having to compute the puzzle hash if we don't have to
        // so check parent and amount first
        if parent != find_parent.as_ref() || amount != find_amount {
            continue;
        }

        let puzzle_hash = tree_hash(a, puzzle);
        if puzzle_hash != find_ph.as_ref() {
            continue;
        }

        // we found the coin!
        return Ok((puzzle, solution));
    }
    Err(ValidationErr(generator_result, ErrorCode::InvalidCondition))
}

#[cfg(test)]
fn u64_to_bytes(n: u64) -> Vec<u8> {
    let mut buf = Vec::<u8>::new();
    buf.extend_from_slice(&n.to_be_bytes());
    if (buf[0] & 0x80) != 0 {
        buf.insert(0, 0);
    } else {
        while buf.len() > 1 && buf[0] == 0 && (buf[1] & 0x80) == 0 {
            buf.remove(0);
        }
    }
    buf
}

#[cfg(test)]
use clvmr::sha2::{Digest, Sha256};

#[cfg(test)]
fn make_dummy_id(seed: u64) -> Bytes32 {
    let mut sha256 = Sha256::new();
    sha256.update(&seed.to_be_bytes());
    sha256.finalize().as_slice().into()
}

#[cfg(test)]
fn make_dummy_puzzle(a: &mut Allocator, seed: u32) -> NodePtr {
    let first = a.new_atom(&seed.to_be_bytes()).unwrap();
    let second = a.new_atom(&(0xffffffff - seed).to_be_bytes()).unwrap();
    a.new_pair(first, second).unwrap()
}

#[cfg(test)]
fn make_coin_spend(
    a: &mut Allocator,
    parent: Bytes32,
    amount: u64,
    puzzle: NodePtr,
    solution: NodePtr,
) -> NodePtr {
    let a1 = a.new_atom(&parent).unwrap();
    let a4 = a.new_atom(&u64_to_bytes(amount)).unwrap();
    let a5 = a.new_pair(solution, a.null()).unwrap();
    let a3 = a.new_pair(a4, a5).unwrap();
    let a2 = a.new_pair(puzzle, a3).unwrap();
    a.new_pair(a1, a2).unwrap()
}

#[cfg(test)]
fn make_invalid_coin_spend(
    a: &mut Allocator,
    parent: NodePtr,
    amount: NodePtr,
    puzzle: NodePtr,
    solution: NodePtr,
) -> NodePtr {
    let a5 = a.new_pair(solution, a.null()).unwrap();
    let a3 = a.new_pair(amount, a5).unwrap();
    let a2 = a.new_pair(puzzle, a3).unwrap();
    a.new_pair(parent, a2).unwrap()
}

#[test]
fn test_find_single_coin() {
    let mut a = Allocator::new();
    let parent = make_dummy_id(1);
    let puzzle1 = make_dummy_puzzle(&mut a, 1);
    let solution1 = make_dummy_puzzle(&mut a, 2);

    let spend1 = make_coin_spend(&mut a, parent, 1337, puzzle1, solution1);
    let spends = a.new_pair(spend1, a.null()).unwrap();
    let generator_output = a.new_pair(spends, a.null()).unwrap();

    // find the coin
    assert_eq!(
        get_puzzle_and_solution_for_coin(
            &a,
            generator_output,
            parent,
            1337,
            tree_hash(&a, puzzle1).into()
        )
        .unwrap(),
        (puzzle1, solution1)
    );

    // wrong parent
    assert_eq!(
        get_puzzle_and_solution_for_coin(
            &a,
            generator_output,
            make_dummy_id(2),
            1337,
            tree_hash(&a, puzzle1).into()
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidCondition
    );

    // wrong amount
    assert_eq!(
        get_puzzle_and_solution_for_coin(
            &a,
            generator_output,
            parent,
            42,
            tree_hash(&a, puzzle1).into()
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidCondition
    );

    // wrong puzzle hash
    assert_eq!(
        get_puzzle_and_solution_for_coin(&a, generator_output, parent, 1337, make_dummy_id(4))
            .unwrap_err()
            .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_parse_coin_spend() {
    let mut a = Allocator::new();
    let parent = make_dummy_id(1);
    let parent_atom = a.new_atom(&parent).unwrap();
    let puzzle1 = make_dummy_puzzle(&mut a, 1);
    let puzzle2 = make_dummy_puzzle(&mut a, 4);
    let solution1 = make_dummy_puzzle(&mut a, 2);
    let amount_atom = a.new_atom(&u64_to_bytes(1337)).unwrap();

    let spend1 = make_coin_spend(&mut a, parent, 1337, puzzle1, solution1);
    assert_eq!(
        parse_coin_spend(&a, spend1).unwrap(),
        (parent.as_ref(), 1337, puzzle1, solution1)
    );

    // this is a spend where the parent is not an atom
    let spend2 = make_invalid_coin_spend(&mut a, puzzle2, amount_atom, puzzle1, solution1);
    assert_eq!(
        parse_coin_spend(&a, spend2).unwrap_err().1,
        ErrorCode::InvalidParentId
    );

    // this is a spend where the amount is not an atom
    let spend3 = make_invalid_coin_spend(&mut a, parent_atom, puzzle2, puzzle1, solution1);
    assert_eq!(
        parse_coin_spend(&a, spend3).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );
}
