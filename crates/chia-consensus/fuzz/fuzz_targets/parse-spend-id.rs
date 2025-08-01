#![no_main]
use libfuzzer_sys::{arbitrary, fuzz_target};

use chia_consensus::messages::SpendId;
use clvm_fuzzing::make_list;
use clvmr::allocator::Allocator;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let mut unstructured = arbitrary::Unstructured::new(data);
    let mode: u8 = unstructured.arbitrary().unwrap_or(0);
    let mut input = make_list(&mut a, &mut unstructured);

    let Ok(s) = SpendId::parse(&a, &mut input, mode) else {
        return;
    };

    match s {
        SpendId::OwnedCoinId(_bytes) => unreachable!(),
        SpendId::CoinId(coinid) => {
            assert_eq!(mode, 0b111);
            assert_eq!(a.atom_len(coinid), 32);
        }
        SpendId::Parent(parent) => {
            assert_eq!(mode, 0b100);
            assert_eq!(a.atom_len(parent), 32);
        }
        SpendId::Puzzle(puzzle) => {
            assert_eq!(mode, 0b010);
            assert_eq!(a.atom_len(puzzle), 32);
        }
        SpendId::Amount(_amount) => {
            assert_eq!(mode, 0b001);
        }
        SpendId::PuzzleAmount(puzzle, _amount) => {
            assert_eq!(a.atom_len(puzzle), 32);
            assert_eq!(mode, 0b011);
        }
        SpendId::ParentAmount(parent, _amount) => {
            assert_eq!(a.atom_len(parent), 32);
            assert_eq!(mode, 0b101);
        }
        SpendId::ParentPuzzle(parent, puzzle) => {
            assert_eq!(a.atom_len(parent), 32);
            assert_eq!(a.atom_len(puzzle), 32);
            assert_eq!(mode, 0b110);
        }
        SpendId::None => assert_eq!(mode, 0),
    }
});
