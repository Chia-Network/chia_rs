use chia_bls::{SecretKey, Signature};
use clvm_utils::Allocate;
use clvmr::{allocator::NodePtr, Allocator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    CreateCoin {
        puzzle_hash: [u8; 32],
        amount: u64,
        memos: Vec<[u8; 32]>,
    },
}

impl Allocate for Condition {
    fn from_clvm(_a: &Allocator, _node: NodePtr) -> clvm_utils::Result<Self> {
        todo!()
    }
    fn to_clvm(&self, a: &mut Allocator) -> clvm_utils::Result<NodePtr> {
        match self {
            Self::CreateCoin {
                puzzle_hash,
                amount,
                memos,
            } => {
                if memos.is_empty() {
                    Allocate::to_clvm(&(51, (*puzzle_hash, (*amount, ()))), a)
                } else {
                    Allocate::to_clvm(&(51, (*puzzle_hash, (*amount, (memos.clone(), ())))), a)
                }
            }
        }
    }
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
