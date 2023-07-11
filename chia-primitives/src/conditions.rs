use chia_bls::{SecretKey, Signature};
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

pub fn sign_agg_sig_me(
    raw_message: &[u8],
    coin_id: &[u8; 32],
    agg_sig_me_extra_data: &[u8; 32],
    secret_key: &SecretKey,
) -> Signature {
    let mut message = Vec::with_capacity(96);
    message.extend(raw_message);
    message.extend(coin_id);
    message.extend(agg_sig_me_extra_data);
    secret_key.sign(&message)
}
