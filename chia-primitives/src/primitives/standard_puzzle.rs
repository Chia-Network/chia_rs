use chia_bls::{PublicKey, SecretKey, Signature};
use clvm_utils::{curry, curry_tree_hash, new_list, tree_hash, tree_hash_atom};
use clvmr::{allocator::NodePtr, reduction::EvalErr, serde::node_from_bytes, Allocator};

use crate::puzzles::{P2_DELEGATED_OR_HIDDEN, P2_DELEGATED_OR_HIDDEN_HASH};

pub fn alloc_standard_puzzle(a: &mut Allocator) -> std::io::Result<NodePtr> {
    node_from_bytes(a, &P2_DELEGATED_OR_HIDDEN)
}

pub fn standard_puzzle_hash(synthetic_key: PublicKey) -> [u8; 32] {
    let synthetic_key = tree_hash_atom(&synthetic_key.to_bytes());
    curry_tree_hash(&P2_DELEGATED_OR_HIDDEN_HASH, &[&synthetic_key])
}

pub fn curry_standard_puzzle(
    a: &mut Allocator,
    node: NodePtr,
    synthetic_key: PublicKey,
) -> Result<NodePtr, EvalErr> {
    let synthetic_key = a.new_atom(&synthetic_key.to_bytes())?;
    curry(a, node, &[synthetic_key])
}

pub fn spend_standard_puzzle(
    a: &mut Allocator,
    coin_id: &[u8; 32],
    conditions: &[NodePtr],
    secret_key: &SecretKey,
    agg_sig_me_extra_data: &[u8; 32],
) -> Result<(NodePtr, Signature), EvalErr> {
    let condition_list = new_list(a, conditions)?;
    let delegated_puzzle = a.new_pair(a.one(), condition_list)?;
    let nil = a.null();
    let solution = new_list(a, &[nil, delegated_puzzle, nil])?;

    let raw_message = tree_hash(a, delegated_puzzle);
    let mut message = Vec::with_capacity(96);
    message.extend(raw_message);
    message.extend(coin_id);
    message.extend(agg_sig_me_extra_data);
    let signature = secret_key.sign(&message);

    Ok((solution, signature))
}
