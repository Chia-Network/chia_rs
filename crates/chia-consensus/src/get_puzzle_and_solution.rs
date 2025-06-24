use crate::validation_error::{atom, check_nil, first, next, rest, ErrorCode, ValidationErr};
use chia_protocol::Coin;
use clvm_utils::{tree_hash_cached, TreeCache};
use clvmr::allocator::{Allocator, Atom, NodePtr};
use clvmr::op_utils::u64_from_bytes;

/// returns parent-coin ID, amount, puzzle-reveal and solution
pub fn parse_coin_spend(
    a: &Allocator,
    coin_spend: NodePtr,
) -> Result<(Atom<'_>, u64, NodePtr, NodePtr), ValidationErr> {
    let parent = atom(a, first(a, coin_spend)?, ErrorCode::InvalidParentId)?;
    let coin_spend = rest(a, coin_spend)?;
    let puzzle = first(a, coin_spend)?;
    let coin_spend = rest(a, coin_spend)?;
    let amount =
        u64_from_bytes(atom(a, first(a, coin_spend)?, ErrorCode::InvalidCoinAmount)?.as_ref());
    let coin_spend = rest(a, coin_spend)?;
    let solution = first(a, coin_spend)?;
    check_nil(a, rest(a, coin_spend)?)?;
    Ok((parent, amount, puzzle, solution))
}

pub fn get_puzzle_and_solution_for_coin(
    a: &Allocator,
    generator_result: NodePtr,
    find_coin: &Coin,
) -> Result<(NodePtr, NodePtr), ValidationErr> {
    // the output from the block generator is a list of CoinSpends
    // with (parent-coin-id puzzle-reveal amount solution)
    // this function is given the generator output and a parent_coin_id, amount
    // and puzzle_hash and it will return the puzzle and solution for that given
    // coin spend, or fail if it cannot be found
    let mut cache = TreeCache::default();
    let mut iter = first(a, generator_result)?;
    while let Some((coin_spend, next)) = next(a, iter)? {
        iter = next;
        // coin_spend is (parent puzzle amount solution)
        let (parent, amount, puzzle, solution) = parse_coin_spend(a, coin_spend)?;

        // we want to avoid having to compute the puzzle hash if we don't have to
        // so check parent and amount first
        if parent.as_ref() != find_coin.parent_coin_info.as_ref() || amount != find_coin.amount {
            continue;
        }

        let puzzle_hash = tree_hash_cached(a, puzzle, &mut cache);
        if puzzle_hash != find_coin.puzzle_hash.into() {
            continue;
        }

        // we found the coin!
        return Ok((puzzle, solution));
    }
    Err(ValidationErr(generator_result, ErrorCode::InvalidCondition))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::flags::{DONT_VALIDATE_SIGNATURE, MEMPOOL_MODE};
    use crate::make_aggsig_final_message::u64_to_bytes;
    use crate::run_block_generator::{run_block_generator2, setup_generator_args};
    use chia_bls::Signature;
    use chia_protocol::Bytes32;
    use chia_sha2::Sha256;
    use clvm_traits::FromClvm;
    use clvm_utils::tree_hash;
    use clvmr::reduction::Reduction;
    use clvmr::serde::node_from_bytes_backrefs;
    use clvmr::{run_program, ChiaDialect};
    use rstest::rstest;
    use std::collections::HashSet;
    use std::fs;

    const MAX_COST: u64 = 11_000_000_000;

    fn make_dummy_id(seed: u64) -> Bytes32 {
        let mut sha256 = Sha256::new();
        sha256.update(seed.to_be_bytes());
        let id: [u8; 32] = sha256.finalize();
        id.into()
    }

    fn make_dummy_puzzle(a: &mut Allocator, seed: u32) -> NodePtr {
        let first = a.new_atom(&seed.to_be_bytes()).unwrap();
        let second = a.new_atom(&(0xffff_ffff - seed).to_be_bytes()).unwrap();
        a.new_pair(first, second).unwrap()
    }

    fn make_coin_spend(
        a: &mut Allocator,
        parent: Bytes32,
        amount: u64,
        puzzle: NodePtr,
        solution: NodePtr,
    ) -> NodePtr {
        let a1 = a.new_atom(&parent).unwrap();
        let a4 = a.new_atom(&u64_to_bytes(amount)).unwrap();
        let a5 = a.new_pair(solution, a.nil()).unwrap();
        let a3 = a.new_pair(a4, a5).unwrap();
        let a2 = a.new_pair(puzzle, a3).unwrap();
        a.new_pair(a1, a2).unwrap()
    }

    fn make_invalid_coin_spend(
        a: &mut Allocator,
        parent: NodePtr,
        amount: NodePtr,
        puzzle: NodePtr,
        solution: NodePtr,
    ) -> NodePtr {
        let a5 = a.new_pair(solution, a.nil()).unwrap();
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
        let spends = a.new_pair(spend1, a.nil()).unwrap();
        let generator_output = a.new_pair(spends, a.nil()).unwrap();

        // find the coin
        assert_eq!(
            get_puzzle_and_solution_for_coin(
                &a,
                generator_output,
                &Coin::new(parent, tree_hash(&a, puzzle1).into(), 1337),
            )
            .unwrap(),
            (puzzle1, solution1)
        );

        // wrong parent
        assert_eq!(
            get_puzzle_and_solution_for_coin(
                &a,
                generator_output,
                &Coin::new(make_dummy_id(2), tree_hash(&a, puzzle1).into(), 1337),
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
                &Coin::new(parent, tree_hash(&a, puzzle1).into(), 42),
            )
            .unwrap_err()
            .1,
            ErrorCode::InvalidCondition
        );

        // wrong puzzle hash
        assert_eq!(
            get_puzzle_and_solution_for_coin(
                &a,
                generator_output,
                &Coin::new(parent, make_dummy_id(4), 1337),
            )
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
            (Atom::Borrowed(&parent), 1337, puzzle1, solution1)
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

    #[rstest]
    #[case("block-1ee588dc")]
    #[case("block-6fe59b24")]
    #[case("block-834752-compressed")]
    #[case("block-834752")]
    #[case("block-834760")]
    #[case("block-834761")]
    #[case("block-834765")]
    #[case("block-834766")]
    #[case("block-834768")]
    #[case("block-b45268ac")]
    #[case("block-c2a8df0d")]
    #[case("block-e5002df2")]
    fn test_get_puzzle_and_solution(#[case] name: &str) {
        let filename = format!("../../generator-tests/{name}.txt");
        println!("file: {filename}");
        let test_file = fs::read_to_string(filename).expect("test file not found");
        let generator = test_file.split_once('\n').expect("invalid test file").0;
        let generator = hex::decode(generator).expect("invalid hex encoded generator");

        let mut a = Allocator::new();
        let blocks: &[&[u8]] = &[];
        let conds = run_block_generator2(
            &mut a,
            &generator,
            blocks,
            MAX_COST,
            MEMPOOL_MODE | DONT_VALIDATE_SIGNATURE,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        )
        .expect("run_block_generator2");

        let mut a2 = Allocator::new();
        let generator_node =
            node_from_bytes_backrefs(&mut a2, &generator).expect("node_from_bytes_backrefs");
        let checkpoint = a2.checkpoint();
        for s in &conds.spends {
            a2.restore_checkpoint(&checkpoint);
            let mut expected_additions: HashSet<(Bytes32, u64)> = s
                .create_coin
                .iter()
                .map(|c| (c.puzzle_hash, c.amount))
                .collect();

            let dialect = &ChiaDialect::new(MEMPOOL_MODE);
            let args = setup_generator_args(&mut a2, blocks).expect("setup_generator_args");
            let Reduction(_, result) =
                run_program(&mut a2, dialect, generator_node, args, MAX_COST)
                    .expect("run_program (generator)");

            let (puzzle, solution) = get_puzzle_and_solution_for_coin(
                &a2,
                result,
                &Coin::new(
                    a.atom(s.parent_id).as_ref().try_into().unwrap(),
                    a.atom(s.puzzle_hash).as_ref().try_into().unwrap(),
                    s.coin_amount,
                ),
            )
            .expect("get_puzzle_and_solution_for_coin");

            let Reduction(_, mut iter) = run_program(&mut a2, dialect, puzzle, solution, MAX_COST)
                .expect("run_program (puzzle)");

            while let Some((c, next)) = next(&a2, iter).expect("next") {
                iter = next;
                // 51 is CREATE_COIN
                if let Ok((_create_coin, (puzzle_hash, (amount, _rest)))) =
                    <(clvm_traits::MatchByte<51>, (Bytes32, (u64, NodePtr)))>::from_clvm(&a2, c)
                {
                    assert!(expected_additions.contains(&(puzzle_hash, amount)));
                    expected_additions.remove(&(puzzle_hash, amount));
                }
            }
            assert!(expected_additions.is_empty());
        }
    }
}
