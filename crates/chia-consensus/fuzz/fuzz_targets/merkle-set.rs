#![no_main]
use chia_consensus::merkle_tree::{MerkleSet, validate_merkle_proof};
use chia_sha2::Sha256;
use libfuzzer_sys::{Corpus, fuzz_target};

fuzz_target!(|leafs_: Vec::<[u8; 32]>| -> Corpus {
    let num_leafs = leafs_.len();
    if num_leafs == 0 {
        return Corpus::Reject;
    }
    let mut leafs = leafs_.clone();

    let tree = MerkleSet::from_leafs(&mut leafs);
    let root = tree.get_root();

    // this is a leaf that's *not* in the tree, to also cover
    // proofs-of-exclusion
    let mut hasher = Sha256::new();
    for leaf in &leafs {
        hasher.update(leaf);
    }
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
    Corpus::Keep
});
