use super::opcodes::{
    parse_opcode, ASSERT_BEFORE_HEIGHT_ABSOLUTE, ASSERT_BEFORE_HEIGHT_RELATIVE,
    ASSERT_BEFORE_SECONDS_ABSOLUTE, ASSERT_BEFORE_SECONDS_RELATIVE, ASSERT_COIN_ANNOUNCEMENT,
    ASSERT_CONCURRENT_PUZZLE, ASSERT_CONCURRENT_SPEND, ASSERT_EPHEMERAL, ASSERT_HEIGHT_ABSOLUTE,
    ASSERT_HEIGHT_RELATIVE, ASSERT_MY_AMOUNT, ASSERT_MY_BIRTH_HEIGHT, ASSERT_MY_BIRTH_SECONDS,
    ASSERT_MY_COIN_ID, ASSERT_MY_PARENT_ID, ASSERT_MY_PUZZLEHASH, ASSERT_PUZZLE_ANNOUNCEMENT,
    ASSERT_SECONDS_ABSOLUTE, ASSERT_SECONDS_RELATIVE, CREATE_COIN, CREATE_COIN_ANNOUNCEMENT,
    CREATE_PUZZLE_ANNOUNCEMENT, REMARK, RESERVE_FEE,
};
use crate::flags::MEMPOOL_MODE;
use crate::validation_error::{first, ErrorCode, ValidationErr};
use chia_sha2::Sha256;
use clvmr::chia_dialect::ENABLE_KECCAK_OPS_OUTSIDE_GUARD;
use clvmr::{Allocator, NodePtr, SExp};

/// computes a hash of the atoms in a CLVM list. Only the `count` first items
/// are considered. Returns the NodePtr to the remainder of the list (may be
/// NIL)
fn hash_atom_list(
    fingerprint: &mut Sha256,
    a: &Allocator,
    mut args: NodePtr,
    mut count: u32,
) -> Result<NodePtr, ValidationErr> {
    while count > 0 {
        let Some((arg, next)) = a.next(args) else {
            return Err(ValidationErr(args, ErrorCode::InvalidCondition));
        };
        args = next;
        count -= 1;
        if !matches!(a.sexp(arg), SExp::Atom) {
            return Err(ValidationErr(arg, ErrorCode::InvalidCondition));
        }
        let buf = a.atom(arg);

        // every atom gets a length prefix, to avoid playing games with the
        // resulting hash.
        // e.g. two adjacent atoms whose concatenation stays the same, but sizes
        // changes. Those cases must be distinguished
        fingerprint.update((buf.as_ref().len() as u32).to_be_bytes());
        fingerprint.update(buf.as_ref());
    }
    Ok(args)
}

/// This functions runs a *trusted*, *dedup* puzzles, i.e. one that has already
/// been fully validated in mempool mode, and returns the conditions fingerprint
/// for it. The conditions fingerprint is a hash of its
/// condition outputs (but only known conditions). This is used for identical
/// spend deduplication to compare whether two spends are identical and can be
/// deduplicated. The condition fingerprint should not be expected to be stable
/// across different chia_rs versions.
/// This function may fail if the puzzle returns a condition that's not
/// supported by DEDUP spends.
pub fn compute_puzzle_fingerprint(
    a: &Allocator,
    conditions: NodePtr,
) -> core::result::Result<[u8; 32], ValidationErr> {
    // keep in mind that the puzzle has already been validated by the mempool,
    // so it's trusted. It's OK to enable features that aren't available yet,
    // because if the puzzle would use them prematurely, the validation would
    // have failed.
    let flags = MEMPOOL_MODE | ENABLE_KECCAK_OPS_OUTSIDE_GUARD;
    let mut iter = conditions;

    let mut fingerprint = Sha256::new();

    while let Some((c, next)) = a.next(iter) {
        iter = next;

        // The fingerprint is internal to the mempool and should take all
        // currently and future known conditions into account. i.e. conditions
        // that will be enabled in future forks.
        let Some(op) = parse_opcode(a, first(a, c)?, flags) else {
            // we just ignore unknown conditions
            continue;
        };

        // since we only run in mempool mode, we don't need to take unknown
        // conditions into account, including the ones with cost. This puzzle is
        // expected to have already passed mempool-mode validation
        match op {
            CREATE_COIN => {
                // CREATE_COIN, puzzle_hash, amount
                let rest = hash_atom_list(&mut fingerprint, a, c, 3)?;

                // make sure to include the hint if present. If it's not present
                // we insert an empty atom instead, to ensure CREATE_COIN always
                // adds 4 atoms to the fingerprint
                if let Ok(memos) = first(a, rest) {
                    if let Ok(hint) = first(a, memos) {
                        if let SExp::Atom = a.sexp(hint) {
                            if a.atom_len(hint) <= 32 {
                                hash_atom_list(&mut fingerprint, a, memos, 1)?;
                            } else {
                                fingerprint.update(0_u32.to_be_bytes());
                            }
                        } else {
                            fingerprint.update(0_u32.to_be_bytes());
                        }
                    } else {
                        fingerprint.update(0_u32.to_be_bytes());
                    }
                } else {
                    fingerprint.update(0_u32.to_be_bytes());
                }
            }

            // These conditions take 1 parameter
            RESERVE_FEE
            | CREATE_COIN_ANNOUNCEMENT
            | ASSERT_COIN_ANNOUNCEMENT
            | CREATE_PUZZLE_ANNOUNCEMENT
            | ASSERT_PUZZLE_ANNOUNCEMENT
            | ASSERT_CONCURRENT_SPEND
            | ASSERT_CONCURRENT_PUZZLE
            | ASSERT_MY_COIN_ID
            | ASSERT_MY_PARENT_ID
            | ASSERT_MY_PUZZLEHASH
            | ASSERT_MY_AMOUNT
            | ASSERT_MY_BIRTH_SECONDS
            | ASSERT_MY_BIRTH_HEIGHT
            | ASSERT_SECONDS_RELATIVE
            | ASSERT_SECONDS_ABSOLUTE
            | ASSERT_HEIGHT_RELATIVE
            | ASSERT_HEIGHT_ABSOLUTE
            | ASSERT_BEFORE_SECONDS_RELATIVE
            | ASSERT_BEFORE_SECONDS_ABSOLUTE
            | ASSERT_BEFORE_HEIGHT_RELATIVE
            | ASSERT_BEFORE_HEIGHT_ABSOLUTE => {
                hash_atom_list(&mut fingerprint, a, c, 2)?;
            }

            // These conditions take no parameters
            ASSERT_EPHEMERAL | REMARK => {
                hash_atom_list(&mut fingerprint, a, c, 1)?;
            }
            _ => {
                return Err(ValidationErr(c, ErrorCode::InvalidConditionOpcode));
            }
        }
    }
    Ok(fingerprint.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_hash_atom_list_single_element() {
        let mut a = Allocator::new();
        let val = a.new_atom(b"foobar").unwrap();
        let list = a.new_pair(val, NodePtr::NIL).unwrap();

        let mut ctx1 = Sha256::new();
        let rest = hash_atom_list(&mut ctx1, &a, list, 1).expect("hash_atom_list");
        assert_eq!(rest, a.nil());

        let mut ctx2 = Sha256::new();
        // length-prefix
        ctx2.update(b"\x00\x00\x00\x06");
        ctx2.update(b"foobar");

        assert_eq!(ctx1.finalize(), ctx2.finalize());
    }

    #[test]
    fn test_hash_atom_list_two_elements() {
        let mut a = Allocator::new();

        let val = a.new_atom(b"bar").unwrap();
        let list1 = a.new_pair(val, NodePtr::NIL).unwrap();
        let val = a.new_atom(b"foo").unwrap();
        let list2 = a.new_pair(val, list1).unwrap();

        // we just care about 1 element
        {
            let mut ctx1 = Sha256::new();
            let rest = hash_atom_list(&mut ctx1, &a, list2, 1).expect("hash_atom_list");
            assert_eq!(rest, list1);

            let mut ctx2 = Sha256::new();
            // length-prefix
            ctx2.update(b"\x00\x00\x00\x03");
            ctx2.update(b"foo");

            assert_eq!(ctx1.finalize(), ctx2.finalize());
        }

        // we just care about 2 elements
        {
            let mut ctx1 = Sha256::new();
            let rest = hash_atom_list(&mut ctx1, &a, list2, 2).expect("hash_atom_list");
            assert_eq!(rest, a.nil());

            let mut ctx2 = Sha256::new();
            // length-prefix
            ctx2.update(b"\x00\x00\x00\x03");
            ctx2.update(b"foo");
            ctx2.update(b"\x00\x00\x00\x03");
            ctx2.update(b"bar");

            assert_eq!(ctx1.finalize(), ctx2.finalize());
        }
    }

    #[test]
    fn test_hash_atom_list_not_enough_items() {
        let mut a = Allocator::new();
        let val = a.new_atom(b"foobar").unwrap();
        let list = a.new_pair(val, NodePtr::NIL).unwrap();

        let mut ctx1 = Sha256::new();

        // we expect 2 elements, but there's only 1
        assert_eq!(
            hash_atom_list(&mut ctx1, &a, list, 2).unwrap_err().1,
            ErrorCode::InvalidCondition
        );
    }

    #[test]
    fn test_hash_atom_list_pair() {
        let mut a = Allocator::new();
        let val = a.new_pair(NodePtr::NIL, NodePtr::NIL).unwrap();
        let list = a.new_pair(val, NodePtr::NIL).unwrap();

        let mut ctx1 = Sha256::new();

        // we expect all elements to be atoms, but we encountered a pair
        assert_eq!(
            hash_atom_list(&mut ctx1, &a, list, 1).unwrap_err().1,
            ErrorCode::InvalidCondition
        );
    }

    #[rstest]
    #[case(&[&ASSERT_MY_AMOUNT.to_le_bytes()[0..1], &[100]], 2)]
    #[case(&[&CREATE_COIN.to_le_bytes()[0..1], b"11111111111111111111111111111111", &[100], &[]], 4)]
    #[case(&[&CREATE_COIN.to_le_bytes()[0..1], b"11111111111111111111111111111111", &[0x10], &[]], 4)]
    #[case(&[&ASSERT_SECONDS_RELATIVE.to_le_bytes()[0..1], &[0x10, 0x10, 0x10]], 2)]
    #[case(&[&CREATE_PUZZLE_ANNOUNCEMENT.to_le_bytes()[0..1], b"11111111111111111111111111111111"], 2)]
    #[case(&[&RESERVE_FEE.to_le_bytes()[0..1], &[98]], 2)]
    fn test_compute_puzzle_fingerprint(#[case] condition: &[&[u8]], #[case] mut args: u32) {
        // build the puzzle as a quoted list with a single condition
        // as well as the expected fingerprint
        let mut ctx = Sha256::new();

        let mut a = Allocator::new();
        let mut cond = NodePtr::NIL;
        for atom in condition {
            ctx.update((atom.len() as u32).to_be_bytes());
            if !atom.is_empty() {
                ctx.update(atom);
            }
            args -= 1;
            if args == 0 {
                break;
            }
        }
        let expect_fingerprint = ctx.finalize();

        // The ChiaLisp list must be built in reverse order
        for atom in condition.iter().rev() {
            let val = a.new_atom(atom).expect("new_atom");
            cond = a.new_pair(val, cond).expect("new_pair");
        }

        let condition_list = a.new_pair(cond, NodePtr::NIL).expect("new_pair");

        let fingerprint =
            compute_puzzle_fingerprint(&a, condition_list).expect("compute_puzzle_fingerprint");

        assert_eq!(fingerprint, expect_fingerprint);
    }

    #[rstest]
    fn test_compute_puzzle_fingerprint_create_coin(#[values(false, true)] with_hint: bool) {
        let opcode: &[u8] = &CREATE_COIN.to_le_bytes()[0..1];
        let puzzle_hash: &[u8] = b"00000000000000000000000000000000";
        let amount: &[u8] = &[0x0f, 0x42, 0x40];
        let hint: &[u8] = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        // build the puzzle as a quoted list with a single condition
        let mut a = Allocator::new();
        let mut cond = NodePtr::NIL;
        if with_hint {
            // the hint is the first element in the list, which is the 4th
            // argument to CREATE_COIN
            let val = a.new_atom(hint).expect("new_atom");
            let memo = a.new_pair(val, NodePtr::NIL).expect("new_pair");
            cond = a.new_pair(memo, cond).expect("new_pair");
        }
        let val = a.new_atom(amount).expect("new_atom");
        cond = a.new_pair(val, cond).expect("new_pair");

        let val = a.new_atom(puzzle_hash).expect("new_atom");
        cond = a.new_pair(val, cond).expect("new_pair");

        let val = a.new_atom(opcode).expect("new_atom");
        cond = a.new_pair(val, cond).expect("new_pair");

        let condition_list = a.new_pair(cond, NodePtr::NIL).expect("new_pair");

        let mut ctx = Sha256::new();
        ctx.update([0, 0, 0, 1]);
        ctx.update([51]);
        ctx.update([0, 0, 0, 32]);
        ctx.update(puzzle_hash);
        ctx.update([0, 0, 0, 3]);
        ctx.update(amount);
        if with_hint {
            ctx.update([0, 0, 0, 32]);
            ctx.update(hint);
        } else {
            // If there is no hint, we encode it as an empty atom
            ctx.update([0, 0, 0, 0]);
        }
        let expect_fingerprint = ctx.finalize();

        let fingerprint =
            compute_puzzle_fingerprint(&a, condition_list).expect("compute_puzzle_fingerprint");

        assert_eq!(fingerprint, expect_fingerprint);
    }
}
