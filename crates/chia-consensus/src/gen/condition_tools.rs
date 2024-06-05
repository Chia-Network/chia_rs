use crate::consensus_constants::ConsensusConstants;
use crate::gen::opcodes::{
    ConditionOpcode, AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT,
    AGG_SIG_PARENT_PUZZLE, AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT,
};
use crate::gen::owned_conditions::OwnedSpend;
use chia_protocol::Bytes;
use chia_protocol::Coin;

pub fn make_aggsig_final_message(
    opcode: ConditionOpcode,
    msg: &[u8],
    spend: &OwnedSpend,
    constants: &ConsensusConstants,
) -> Vec<u8> {
    let mut result = msg.to_vec();
    result.extend(match opcode {
        AGG_SIG_PARENT => [
            spend.parent_id.as_slice(),
            constants.agg_sig_parent_additional_data.as_slice(),
        ]
        .concat(),
        AGG_SIG_PUZZLE => [
            spend.puzzle_hash.as_slice(),
            constants.agg_sig_puzzle_additional_data.as_slice(),
        ]
        .concat(),
        AGG_SIG_AMOUNT => [
            u64_to_bytes(spend.coin_amount).as_slice(),
            constants.agg_sig_amount_additional_data.as_slice(),
        ]
        .concat(),
        AGG_SIG_PUZZLE_AMOUNT => [
            spend.puzzle_hash.as_slice(),
            u64_to_bytes(spend.coin_amount).as_slice(),
            constants.agg_sig_puzzle_amount_additional_data.as_slice(),
        ]
        .concat(),
        AGG_SIG_PARENT_AMOUNT => [
            spend.parent_id.as_slice(),
            u64_to_bytes(spend.coin_amount).as_slice(),
            constants.agg_sig_parent_amount_additional_data.as_slice(),
        ]
        .concat(),
        AGG_SIG_PARENT_PUZZLE => [
            spend.parent_id.as_slice(),
            spend.puzzle_hash.as_slice(),
            constants.agg_sig_parent_puzzle_additional_data.as_slice(),
        ]
        .concat(),
        AGG_SIG_ME => {
            let coin: Coin = Coin::new(spend.parent_id, spend.puzzle_hash, spend.coin_amount);
            [
                coin.coin_id().as_slice(),
                constants.agg_sig_me_additional_data.as_slice(),
            ]
            .concat()
        }
        _ => Vec::<u8>::new(),
    });

    result
}

pub fn u64_to_bytes(val: u64) -> Bytes {
    let amount_bytes: [u8; 8] = val.to_be_bytes();
    if val >= 0x8000_0000_0000_0000_u64 {
        let mut ret = Vec::<u8>::new();
        ret.push(0_u8);
        ret.extend(amount_bytes);
        Bytes::new(ret)
    } else {
        let start = match val {
            n if n >= 0x0080_0000_0000_0000_u64 => 0,
            n if n >= 0x8000_0000_0000_u64 => 1,
            n if n >= 0x0080_0000_0000_u64 => 2,
            n if n >= 0x8000_0000_u64 => 3,
            n if n >= 0x0080_0000_u64 => 4,
            n if n >= 0x8000_u64 => 5,
            n if n >= 0x80_u64 => 6,
            n if n > 0 => 7,
            _ => 8,
        };
        Bytes::new(amount_bytes[start..].to_vec())
    }
}
