use chia_bls::{SecretKey, Signature};
use clvm_utils::{new_list, tree_hash};
use clvmr::{allocator::NodePtr, reduction::EvalErr, Allocator};

pub fn spend_standard_puzzle(
    a: &mut Allocator,
    coin_id: &[u8; 32],
    secret_key: &SecretKey,
    agg_sig_me_extra_data: &[u8; 32],
    conditions: &[NodePtr],
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
