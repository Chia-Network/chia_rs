use chia_bls::{SecretKey, Signature};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    CreateCoin {
        puzzle_hash: [u8; 32],
        amount: u64,
        memos: Vec<[u8; 32]>,
    },
}

pub fn sign_agg_sig_me(
    secret_key: &SecretKey,
    raw_message: &[u8],
    coin_id: &[u8; 32],
    agg_sig_me_extra_data: &[u8; 32],
) -> Signature {
    let mut message = Vec::with_capacity(96);
    message.extend(raw_message);
    message.extend(coin_id);
    message.extend(agg_sig_me_extra_data);
    secret_key.sign(&message)
}
