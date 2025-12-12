#![no_main]
use chia_consensus::merkle_tree::{MerkleSet, validate_merkle_proof};
use chia_sha2::Sha256;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut input = data;
    let num_leafs = input.len() / 32;
    let mut leafs = Vec::<[u8; 32]>::with_capacity(num_leafs);
    for _ in 0..num_leafs {
        leafs.push(input[..32].try_into().unwrap());
        input = &input[32..];
    }

    let tree = MerkleSet::from_leafs(&mut leafs);
    let root = tree.get_root();

    // this is a leaf that's *not* in the tree, to also cover
    // proofs-of-exclusion
    let mut hasher = Sha256::new();
    hasher.update(data);
    leafs.push(hasher.finalize());

    for (idx, item) in leafs.iter().enumerate() {
        let expect_included = idx < num_leafs;
        let (included, proof) = tree.generate_proof(item).expect("failed to generate proof");
        assert_eq!(included, expect_included);
        let rebuilt = MerkleSet::from_proof(&proof).expect("failed to parse proof");
        let (included, _junk) = rebuilt
            .generate_proof(item)
            .expect("failed to validate proof");
        assert_eq!(rebuilt.get_root(), root);
        assert_eq!(included, expect_included);
        assert!(
            validate_merkle_proof(&proof, item, &root).expect("proof failed") == expect_included
        );
    }
});
