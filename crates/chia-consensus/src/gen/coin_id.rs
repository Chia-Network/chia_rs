use chia_protocol::Bytes32;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::sha2::{Digest, Sha256};

pub fn compute_coin_id(
    a: &Allocator,
    parent_id: NodePtr,
    puzzle_hash: NodePtr,
    amount: &[u8],
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(a.atom(parent_id));
    hasher.update(a.atom(puzzle_hash));
    hasher.update(amount);
    let coin_id: [u8; 32] = hasher.finalize().into();
    coin_id.into()
}

// from chia.types.blockchain_format.coin import Coin
// Coin(b"abababababababababababababababab", b"11111111111111111111111111111111", 123).name()
// <bytes32: d82ed74b945e6a140ffecda9a619c30c323cdf2053a58dae8922c0c15a87646e>

// Coin(b"abababababababababababababababab", b"11111111111111111111111111111111", 3).name()
// <bytes32: b9cac8f1b15bce73ad14f39451dac46f73494e70f23df2d8fdaddf26cfd83468>

// Coin(b"babababababababababababababababa", b"11111111111111111111111111111111", 3).name()
// <bytes32: 0b85377e9da24041560ee2e1db76bfa86afdb0486b6bed98428e2b35536fdf97>

#[test]
fn test_compute_coin_id() {
    let mut a = Allocator::new();
    let parent_id1 = a
        .new_atom(&[
            0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62,
            0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62,
            0x61, 0x62, 0x61, 0x62,
        ])
        .unwrap();
    let parent_id2 = a
        .new_atom(&[
            0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61,
            0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61, 0x62, 0x61,
            0x62, 0x61, 0x62, 0x61,
        ])
        .unwrap();
    let puzzle_hash1 = a
        .new_atom(&[
            0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31,
            0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31,
            0x31, 0x31, 0x31, 0x31,
        ])
        .unwrap();

    let coin_id = &[
        0xd8, 0x2e, 0xd7, 0x4b, 0x94, 0x5e, 0x6a, 0x14, 0x0f, 0xfe, 0xcd, 0xa9, 0xa6, 0x19, 0xc3,
        0x0c, 0x32, 0x3c, 0xdf, 0x20, 0x53, 0xa5, 0x8d, 0xae, 0x89, 0x22, 0xc0, 0xc1, 0x5a, 0x87,
        0x64, 0x6e,
    ];
    assert_eq!(
        compute_coin_id(&a, parent_id1, puzzle_hash1, &[123]).as_ref(),
        coin_id
    );

    let coin_id = &[
        0xb9, 0xca, 0xc8, 0xf1, 0xb1, 0x5b, 0xce, 0x73, 0xad, 0x14, 0xf3, 0x94, 0x51, 0xda, 0xc4,
        0x6f, 0x73, 0x49, 0x4e, 0x70, 0xf2, 0x3d, 0xf2, 0xd8, 0xfd, 0xad, 0xdf, 0x26, 0xcf, 0xd8,
        0x34, 0x68,
    ];
    assert_eq!(
        compute_coin_id(&a, parent_id1, puzzle_hash1, &[3]).as_ref(),
        coin_id
    );

    let coin_id = &[
        0x0b, 0x85, 0x37, 0x7e, 0x9d, 0xa2, 0x40, 0x41, 0x56, 0x0e, 0xe2, 0xe1, 0xdb, 0x76, 0xbf,
        0xa8, 0x6a, 0xfd, 0xb0, 0x48, 0x6b, 0x6b, 0xed, 0x98, 0x42, 0x8e, 0x2b, 0x35, 0x53, 0x6f,
        0xdf, 0x97,
    ];
    assert_eq!(
        compute_coin_id(&a, parent_id2, puzzle_hash1, &[3]).as_ref(),
        coin_id
    );
}
