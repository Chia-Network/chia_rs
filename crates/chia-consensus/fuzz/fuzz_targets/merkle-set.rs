#![no_main]
use chia_consensus::merkle_tree::MerkleSet;
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

    for item in &leafs {
        let (true, proof) = tree.generate_proof(item).expect("failed to generate proof") else {
            panic!("item is expected to exist");
        };
        let rebuilt = MerkleSet::from_proof(&proof).expect("failed to parse proof");
        assert!(
            rebuilt
                .generate_proof(item)
                .expect("failed to validate proof")
                .0
        );
        assert_eq!(rebuilt.get_root(), tree.get_root());
    }
});
