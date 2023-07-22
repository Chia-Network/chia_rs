use chia_bls::{SecretKey, Signature};
use clvm_utils::{clvm_list, match_list, match_tuple, Error, FromClvm, LazyNode, Result, ToClvm};
use clvmr::{
    allocator::{NodePtr, SExp},
    op_utils::nullp,
    Allocator,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    CreateCoin {
        puzzle_hash: [u8; 32],
        amount: u64,
        memos: Vec<[u8; 32]>,
    },
    CreateCoinAnnouncement {
        message: [u8; 32],
    },
    AssertCoinAnnouncement {
        announcement_id: [u8; 32],
    },
    CreatePuzzleAnnouncement {
        message: [u8; 32],
    },
    AssertPuzzleAnnouncement {
        announcement_id: [u8; 32],
    },
}

impl FromClvm for Condition {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        let (code, LazyNode(args)) = <match_tuple!(u8, LazyNode)>::from_clvm(a, node)?;

        match code {
            51 => {
                let value = <match_tuple!([u8; 32], u64, LazyNode)>::from_clvm(a, args)?;
                let memo_node = value.1 .1 .0;
                Ok(Condition::CreateCoin {
                    puzzle_hash: value.0,
                    amount: value.1 .0,
                    memos: match a.sexp(memo_node) {
                        SExp::Atom() => {
                            if nullp(a, memo_node) {
                                Vec::new()
                            } else {
                                return Err(Error::ExpectedNil(memo_node));
                            }
                        }
                        SExp::Pair(..) => {
                            let memo_value = <match_list!(Vec<[u8; 32]>)>::from_clvm(a, memo_node)?;
                            memo_value.0
                        }
                    },
                })
            }
            60 => {
                let value = <match_list!([u8; 32])>::from_clvm(a, args)?;
                Ok(Condition::CreateCoinAnnouncement { message: value.0 })
            }
            61 => {
                let value = <match_list!([u8; 32])>::from_clvm(a, args)?;
                Ok(Condition::AssertCoinAnnouncement {
                    announcement_id: value.0,
                })
            }
            62 => {
                let value = <match_list!([u8; 32])>::from_clvm(a, args)?;
                Ok(Condition::CreatePuzzleAnnouncement { message: value.0 })
            }
            63 => {
                let value = <match_list!([u8; 32])>::from_clvm(a, args)?;
                Ok(Condition::AssertPuzzleAnnouncement {
                    announcement_id: value.0,
                })
            }
            _ => Err(Error::Reason(format!("unknown condition code {}", code))),
        }
    }
}

impl ToClvm for Condition {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        match self {
            Self::CreateCoin {
                puzzle_hash,
                amount,
                memos,
            } => {
                if memos.is_empty() {
                    clvm_list!(51, puzzle_hash, amount).to_clvm(a)
                } else {
                    clvm_list!(51, puzzle_hash, amount, memos).to_clvm(a)
                }
            }
            Self::CreateCoinAnnouncement { message } => clvm_list!(60, message).to_clvm(a),
            Self::AssertCoinAnnouncement { announcement_id } => {
                clvm_list!(61, announcement_id).to_clvm(a)
            }
            Self::CreatePuzzleAnnouncement { message } => clvm_list!(62, message).to_clvm(a),
            Self::AssertPuzzleAnnouncement { announcement_id } => {
                clvm_list!(63, announcement_id).to_clvm(a)
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
