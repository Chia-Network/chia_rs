use crate::error::{Error, Result};
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use chia_puzzle_types::singleton::{SingletonArgs, SingletonSolution, SingletonStruct};
use chia_puzzle_types::Proof;
use chia_puzzles::SINGLETON_TOP_LAYER_V1_1_HASH;
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::CurriedProgram;
use clvm_utils::TreeHash;
use clvm_utils::{tree_hash, tree_hash_atom, tree_hash_pair};
use clvmr::allocator::{Allocator, NodePtr};

// TODO: replace this with a generic function to compute the hash of curried
// puzzles
const OP_QUOTE: u8 = 1;
const OP_APPLY: u8 = 2;
const OP_CONS: u8 = 4;
fn curry_single_arg(arg_hash: TreeHash, rest: TreeHash) -> TreeHash {
    tree_hash_pair(
        tree_hash_atom(&[OP_CONS]),
        tree_hash_pair(
            tree_hash_pair(tree_hash_atom(&[OP_QUOTE]), arg_hash),
            tree_hash_pair(rest, tree_hash_atom(&[])),
        ),
    )
}

fn curry_and_treehash(inner_puzzle_hash: &Bytes32, singleton_struct: &SingletonStruct) -> Bytes32 {
    let singleton_struct_hash = tree_hash_pair(
        tree_hash_atom(&singleton_struct.mod_hash),
        tree_hash_pair(
            tree_hash_atom(&singleton_struct.launcher_id),
            tree_hash_atom(&singleton_struct.launcher_puzzle_hash),
        ),
    );

    let args_hash = tree_hash_atom(&[OP_QUOTE]);
    let args_hash = curry_single_arg((*inner_puzzle_hash).into(), args_hash);
    let args_hash = curry_single_arg(singleton_struct_hash, args_hash);

    tree_hash_pair(
        tree_hash_atom(&[OP_APPLY]),
        tree_hash_pair(
            tree_hash_pair(
                tree_hash_atom(&[OP_QUOTE]),
                singleton_struct.mod_hash.into(),
            ),
            tree_hash_pair(args_hash, tree_hash_atom(&[])),
        ),
    )
    .into()
}

/// given a puzzle, solution and new coin of a singleton
/// this function validates the lineage proof and returns a new
/// solution spending a new coin ID.
/// The existing coin to be spent and the new coin's parent must also be passed in
/// for validation.
pub fn fast_forward_singleton(
    a: &mut Allocator,
    puzzle: NodePtr,
    solution: NodePtr,
    coin: &Coin,       // the current coin being spent (for validation)
    new_coin: &Coin,   // the new coin to spend
    new_parent: &Coin, // the parent coin of the new coin being spent
) -> Result<NodePtr> {
    // a coin with an even amount is not a valid singleton
    // as defined by singleton_top_layer_v1_1.clsp
    if (coin.amount & 1) == 0 || (new_parent.amount & 1) == 0 || (new_coin.amount & 1) == 0 {
        return Err(Error::CoinAmountEven);
    }

    // we can only fast-forward spends of singletons whose puzzle hash doesn't
    // change
    if coin.puzzle_hash != new_parent.puzzle_hash || coin.puzzle_hash != new_coin.puzzle_hash {
        return Err(Error::PuzzleHashMismatch);
    }

    let singleton = CurriedProgram::<NodePtr, SingletonArgs<NodePtr>>::from_clvm(a, puzzle)?;
    let mut new_solution = SingletonSolution::<NodePtr>::from_clvm(a, solution)?;

    let Proof::Lineage(lineage_proof) = &mut new_solution.lineage_proof else {
        return Err(Error::ExpectedLineageProof);
    };

    // this is the tree hash of the singleton top layer puzzle
    // the tree hash of singleton_top_layer_v1_1.clsp
    if singleton.args.singleton_struct.mod_hash.as_ref() != SINGLETON_TOP_LAYER_V1_1_HASH {
        return Err(Error::NotSingletonModHash);
    }

    // also make sure the actual mod-hash of this puzzle matches the
    // singleton_top_layer_v1_1.clsp
    let mod_hash = tree_hash(a, singleton.program);
    if mod_hash != SINGLETON_TOP_LAYER_V1_1_HASH.into() {
        return Err(Error::NotSingletonModHash);
    }

    // if the current solution to the puzzle doesn't match the coin amount, it's
    // an invalid spend. Don't try to fast-forward it
    if coin.amount != new_solution.amount {
        return Err(Error::CoinAmountMismatch);
    }

    // given the parent's parent, the parent's inner puzzle and parent's amount,
    // we can compute the hash of the curried inner puzzle for our parent coin
    let parent_puzzle_hash = curry_and_treehash(
        &lineage_proof.parent_inner_puzzle_hash,
        &singleton.args.singleton_struct,
    );

    // now that we know the parent coin's puzzle hash, we have all the pieces to
    // compute the coin being spent (before the fast-forward).
    let parent_coin = Coin {
        parent_coin_info: lineage_proof.parent_parent_coin_info,
        puzzle_hash: parent_puzzle_hash,
        amount: lineage_proof.parent_amount,
    };

    if parent_coin.coin_id() != coin.parent_coin_info {
        return Err(Error::ParentCoinMismatch);
    }

    let inner_puzzle_hash = tree_hash(a, singleton.args.inner_puzzle);
    if inner_puzzle_hash != lineage_proof.parent_inner_puzzle_hash.into() {
        return Err(Error::InnerPuzzleHashMismatch);
    }

    let puzzle_hash = tree_hash(a, puzzle);

    if puzzle_hash != new_parent.puzzle_hash.into() || puzzle_hash != coin.puzzle_hash.into() {
        // we can only fast-forward if the puzzle hash match the new coin
        // the spend is assumed to be valied already, so we don't check it
        // against the original coin being spent
        return Err(Error::PuzzleHashMismatch);
    }

    // update the solution to use the new parent coin's information
    lineage_proof.parent_parent_coin_info = new_parent.parent_coin_info;
    lineage_proof.parent_amount = new_parent.amount;
    new_solution.amount = new_coin.amount;

    let expected_new_parent = new_parent.coin_id();

    if new_coin.parent_coin_info != expected_new_parent {
        return Err(Error::CoinMismatch);
    }

    Ok(new_solution.to_clvm(a)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conditions::MempoolVisitor;
    use crate::conditions::{parse_conditions, ParseState, SpendBundleConditions, SpendConditions};
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::spend_visitor::SpendVisitor;
    use crate::validation_error::ValidationErr;
    use chia_protocol::Bytes32;
    use chia_protocol::Coin;
    use chia_protocol::CoinSpend;
    use chia_traits::streamable::Streamable;
    use clvm_traits::ToClvm;
    use clvm_utils::tree_hash;
    use clvmr::allocator::Allocator;
    use clvmr::chia_dialect::ChiaDialect;
    use clvmr::reduction::Reduction;
    use clvmr::run_program::run_program;
    use clvmr::serde::{node_from_bytes, node_to_bytes};
    use hex_literal::hex;
    use rstest::rstest;
    use std::fs;
    use std::sync::Arc;

    pub fn run_puzzle(
        a: &mut Allocator,
        puzzle: &[u8],
        solution: &[u8],
        parent_id: &[u8],
        amount: u64,
    ) -> core::result::Result<SpendBundleConditions, ValidationErr> {
        let puzzle = node_from_bytes(a, puzzle)?;
        let solution = node_from_bytes(a, solution)?;

        let dialect = ChiaDialect::new(0);
        let max_cost = 11_000_000_000;
        let Reduction(clvm_cost, conditions) =
            run_program(a, &dialect, puzzle, solution, max_cost)?;

        let mut ret = SpendBundleConditions {
            removal_amount: amount as u128,
            ..Default::default()
        };
        let mut state = ParseState::default();

        let puzzle_hash = tree_hash(a, puzzle);
        let coin_id = Arc::<Bytes32>::new(
            Coin {
                parent_coin_info: parent_id.try_into().unwrap(),
                puzzle_hash: puzzle_hash.into(),
                amount,
            }
            .coin_id(),
        );

        let mut spend = SpendConditions::new(
            a.new_atom(parent_id)?,
            amount,
            a.new_atom(&puzzle_hash)?,
            coin_id,
            clvm_cost,
        );

        let mut visitor = MempoolVisitor::new_spend(&mut spend);

        let mut cost_left = max_cost - clvm_cost;
        parse_conditions(
            a,
            &mut ret,
            &mut state,
            spend,
            conditions,
            0,
            &mut cost_left,
            &TEST_CONSTANTS,
            &mut visitor,
        )?;
        ret.cost = max_cost - cost_left;
        Ok(ret)
    }

    // this test loads CoinSpends from file (Coin, puzzle, solution)-triples
    // and "fast-forwards" the spend onto a few different parent-parent coins
    // and ensures the spends are still valid
    #[rstest]
    #[case("e3c0")]
    #[case("bb13")]
    fn test_fast_forward(
        #[case] spend_file: &str,
        #[values(
            "abababababababababababababababababababababababababababababababab",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        )]
        new_parents_parent: &str,
        #[values(0, 1, 3, 5)] new_amount: u64,
        #[values(0, 1, 3, 5)] prev_amount: u64,
    ) {
        let spend_bytes =
            fs::read(format!("../../ff-tests/{spend_file}.spend")).expect("read file");
        let spend = CoinSpend::from_bytes(&spend_bytes).expect("parse CoinSpend");
        let new_parents_parent = hex::decode(new_parents_parent).unwrap();

        let mut a = Allocator::new_limited(500_000_000);
        let puzzle = spend.puzzle_reveal.to_clvm(&mut a).expect("to_clvm");
        let solution = spend.solution.to_clvm(&mut a).expect("to_clvm");
        let puzzle_hash = Bytes32::from(tree_hash(&a, puzzle));

        let new_parent_coin = Coin {
            parent_coin_info: new_parents_parent.try_into().unwrap(),
            puzzle_hash,
            amount: if prev_amount == 0 {
                spend.coin.amount
            } else {
                prev_amount
            },
        };

        let new_coin = Coin {
            parent_coin_info: new_parent_coin.coin_id(),
            puzzle_hash,
            amount: if new_amount == 0 {
                spend.coin.amount
            } else {
                new_amount
            },
        };

        // perform fast-forward
        let new_solution = fast_forward_singleton(
            &mut a,
            puzzle,
            solution,
            &spend.coin,
            &new_coin,
            &new_parent_coin,
        )
        .expect("fast-forward");
        let new_solution = node_to_bytes(&a, new_solution).expect("serialize new solution");

        // run original spend
        let conditions1 = run_puzzle(
            &mut a,
            spend.puzzle_reveal.as_slice(),
            spend.solution.as_slice(),
            &spend.coin.parent_coin_info,
            spend.coin.amount,
        )
        .expect("run_puzzle");

        // run new spend
        let conditions2 = run_puzzle(
            &mut a,
            spend.puzzle_reveal.as_slice(),
            new_solution.as_slice(),
            &new_coin.parent_coin_info,
            new_coin.amount,
        )
        .expect("run_puzzle");

        assert!(conditions1.spends[0].create_coin == conditions2.spends[0].create_coin);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn run_ff_test(
        mutate: fn(&mut Allocator, &mut Coin, &mut Coin, &mut Coin, &mut Vec<u8>, &mut Vec<u8>),
        expected_err: Error,
    ) {
        let spend_bytes = fs::read("../../ff-tests/e3c0.spend").expect("read file");
        let mut spend = CoinSpend::from_bytes(&spend_bytes).expect("parse CoinSpend");
        let new_parents_parent: &[u8] =
            &hex!("abababababababababababababababababababababababababababababababab");

        let mut a = Allocator::new_limited(500_000_000);
        let puzzle = spend.puzzle_reveal.to_clvm(&mut a).expect("to_clvm");
        let puzzle_hash = Bytes32::from(tree_hash(&a, puzzle));

        let mut new_parent_coin = Coin {
            parent_coin_info: new_parents_parent.try_into().unwrap(),
            puzzle_hash,
            amount: spend.coin.amount,
        };

        let mut new_coin = Coin {
            parent_coin_info: new_parent_coin.coin_id(),
            puzzle_hash,
            amount: spend.coin.amount,
        };

        let mut puzzle = spend.puzzle_reveal.as_slice().to_vec();
        let mut solution = spend.solution.as_slice().to_vec();
        mutate(
            &mut a,
            &mut spend.coin,
            &mut new_coin,
            &mut new_parent_coin,
            &mut puzzle,
            &mut solution,
        );

        let puzzle = node_from_bytes(&mut a, puzzle.as_slice()).expect("to_clvm");
        let solution = node_from_bytes(&mut a, solution.as_slice()).expect("to_clvm");

        // attempt fast-forward
        assert_eq!(
            fast_forward_singleton(
                &mut a,
                puzzle,
                solution,
                &spend.coin,
                &new_coin,
                &new_parent_coin
            )
            .unwrap_err(),
            expected_err
        );
    }

    #[test]
    fn test_even_amount() {
        run_ff_test(
            |_a, coin, _new_coin, _new_parent, _puzzle, _solution| {
                coin.amount = 2;
            },
            Error::CoinAmountEven,
        );

        run_ff_test(
            |_a, _coin, new_coin, _new_parent, _puzzle, _solution| {
                new_coin.amount = 2;
            },
            Error::CoinAmountEven,
        );

        run_ff_test(
            |_a, _coin, _new_coin, new_parent, _puzzle, _solution| {
                new_parent.amount = 2;
            },
            Error::CoinAmountEven,
        );
    }

    #[test]
    fn test_amount_mismatch() {
        run_ff_test(
            |_a, coin, _new_coin, _new_parent, _puzzle, _solution| {
                coin.amount = 3;
            },
            Error::CoinAmountMismatch,
        );
    }

    fn parse_solution(a: &mut Allocator, solution: &[u8]) -> SingletonSolution<NodePtr> {
        let new_solution = node_from_bytes(a, solution).expect("parse solution");
        let solution = SingletonSolution::from_clvm(a, new_solution).expect("parse solution");
        assert!(matches!(solution.lineage_proof, Proof::Lineage(_)));
        solution
    }

    fn serialize_solution(a: &mut Allocator, solution: &SingletonSolution<NodePtr>) -> Vec<u8> {
        let new_solution = solution.to_clvm(a).expect("to_clvm");
        node_to_bytes(a, new_solution).expect("serialize solution")
    }

    fn parse_singleton(
        a: &mut Allocator,
        puzzle: &[u8],
    ) -> CurriedProgram<NodePtr, SingletonArgs<NodePtr>> {
        let puzzle = node_from_bytes(a, puzzle).expect("parse puzzle");
        CurriedProgram::<NodePtr, SingletonArgs<NodePtr>>::from_clvm(a, puzzle).expect("uncurry")
    }

    fn serialize_singleton(
        a: &mut Allocator,
        singleton: &CurriedProgram<NodePtr, SingletonArgs<NodePtr>>,
    ) -> Vec<u8> {
        let puzzle = singleton.to_clvm(a).expect("to_clvm");
        node_to_bytes(a, puzzle).expect("serialize puzzle")
    }

    #[test]
    fn test_invalid_lineage_proof_parent() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, _puzzle, solution| {
                let mut new_solution = parse_solution(a, solution);

                let Proof::Lineage(lineage_proof) = &mut new_solution.lineage_proof else {
                    unreachable!();
                };

                // corrupt the lineage proof
                lineage_proof.parent_parent_coin_info = Bytes32::from(hex!(
                    "fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe"
                ));

                *solution = serialize_solution(a, &new_solution);
            },
            Error::ParentCoinMismatch,
        );
    }

    #[test]
    fn test_invalid_lineage_proof_parent_amount() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, _puzzle, solution| {
                let mut new_solution = parse_solution(a, solution);

                let Proof::Lineage(lineage_proof) = &mut new_solution.lineage_proof else {
                    unreachable!();
                };

                // corrupt the lineage proof
                lineage_proof.parent_amount = 11;

                *solution = serialize_solution(a, &new_solution);
            },
            Error::ParentCoinMismatch,
        );
    }

    #[test]
    fn test_invalid_lineage_proof_parent_inner_ph() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, _puzzle, solution| {
                let mut new_solution = parse_solution(a, solution);

                let Proof::Lineage(lineage_proof) = &mut new_solution.lineage_proof else {
                    unreachable!();
                };

                // corrupt the lineage proof
                lineage_proof.parent_inner_puzzle_hash = Bytes32::from(hex!(
                    "fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe"
                ));

                *solution = serialize_solution(a, &new_solution);
            },
            Error::ParentCoinMismatch,
        );
    }

    #[test]
    fn test_invalid_lineage_proof_parent_inner_ph_with_coin() {
        run_ff_test(
            |a, coin, new_coin, new_parent, puzzle, solution| {
                let mut new_solution = parse_solution(a, solution);
                let singleton = parse_singleton(a, puzzle);

                let Proof::Lineage(lineage_proof) = &mut new_solution.lineage_proof else {
                    unreachable!();
                };

                // corrupt the lineage proof
                lineage_proof.parent_inner_puzzle_hash = Bytes32::from(hex!(
                    "fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe"
                ));

                // adjust the coins puzzle hashes to match
                let parent_puzzle_hash = curry_and_treehash(
                    &lineage_proof.parent_inner_puzzle_hash,
                    &singleton.args.singleton_struct,
                );

                *new_parent = Coin {
                    parent_coin_info: lineage_proof.parent_parent_coin_info,
                    puzzle_hash: parent_puzzle_hash,
                    amount: lineage_proof.parent_amount,
                };

                *solution = serialize_solution(a, &new_solution);

                new_coin.puzzle_hash = parent_puzzle_hash;

                coin.parent_coin_info = new_parent.coin_id();
                coin.puzzle_hash = parent_puzzle_hash;
            },
            Error::InnerPuzzleHashMismatch,
        );
    }

    #[test]
    fn test_invalid_puzzle_hash() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, puzzle, _solution| {
                let mut singleton = parse_singleton(a, puzzle);

                singleton.program = a.nil();

                *puzzle = serialize_singleton(a, &singleton);
            },
            Error::NotSingletonModHash,
        );
    }

    #[test]
    fn test_invalid_singleton_struct_puzzle_hash() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, puzzle, _solution| {
                let mut singleton = parse_singleton(a, puzzle);

                singleton.args.singleton_struct.mod_hash = Bytes32::from(hex!(
                    "fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe"
                ));

                *puzzle = serialize_singleton(a, &singleton);
            },
            Error::NotSingletonModHash,
        );
    }
}
