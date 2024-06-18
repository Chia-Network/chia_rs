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
    let mut result = Vec::<u8>::with_capacity(msg.len() + 96);
    result.extend(msg);
    match opcode {
        AGG_SIG_PARENT => {
            result.extend(spend.parent_id.as_slice());
            result.extend(constants.agg_sig_parent_additional_data.as_slice());
        }
        AGG_SIG_PUZZLE => {
            result.extend(spend.puzzle_hash.as_slice());
            result.extend(constants.agg_sig_puzzle_additional_data.as_slice());
        }
        AGG_SIG_AMOUNT => {
            result.extend(u64_to_bytes(spend.coin_amount).as_slice());
            result.extend(constants.agg_sig_amount_additional_data.as_slice());
        }
        AGG_SIG_PUZZLE_AMOUNT => {
            result.extend(spend.puzzle_hash.as_slice());
            result.extend(u64_to_bytes(spend.coin_amount).as_slice());
            result.extend(constants.agg_sig_puzzle_amount_additional_data.as_slice());
        }
        AGG_SIG_PARENT_AMOUNT => {
            result.extend(spend.parent_id.as_slice());
            result.extend(u64_to_bytes(spend.coin_amount).as_slice());
            result.extend(constants.agg_sig_parent_amount_additional_data.as_slice());
        }
        AGG_SIG_PARENT_PUZZLE => {
            result.extend(spend.parent_id.as_slice());
            result.extend(spend.puzzle_hash.as_slice());
            result.extend(constants.agg_sig_parent_puzzle_additional_data.as_slice());
        }
        AGG_SIG_ME => {
            let coin: Coin = Coin::new(spend.parent_id, spend.puzzle_hash, spend.coin_amount);

            result.extend(coin.coin_id().as_slice());
            result.extend(constants.agg_sig_me_additional_data.as_slice());
        }
        _ => return result,
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::make_allocator;
    use clvmr::chia_dialect::LIMIT_HEAP;
    use clvmr::Allocator;

    #[test]
    fn test_validate_u64() {
        let mut a: Allocator = make_allocator(LIMIT_HEAP);
        for v in 0..10000 {
            let ptr = a.new_small_number(v).expect("valid u64");
            assert_eq!(a.atom(ptr).as_ref(), u64_to_bytes(v as u64).as_slice())
        }
        for v in 18446744073709551615_u64 - 1000..18446744073709551615 {
            let ptr = a.new_number(v.into()).expect("valid u64");
            assert_eq!(a.atom(ptr).as_ref(), u64_to_bytes(v).as_slice())
        }
    }
}
