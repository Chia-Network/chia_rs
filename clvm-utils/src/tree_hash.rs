use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::serde::node_from_bytes_backrefs;
use clvmr::sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io;

enum TreeOp {
    SExp(NodePtr),
    Cons,
    ConsAddCache(NodePtr),
}

pub fn tree_hash_atom(bytes: &[u8]) -> [u8; 32] {
    let mut sha256 = Sha256::new();
    sha256.update([1]);
    sha256.update(bytes);
    sha256.finalize().into()
}

pub fn tree_hash_pair(first: [u8; 32], rest: [u8; 32]) -> [u8; 32] {
    let mut sha256 = Sha256::new();
    sha256.update([2]);
    sha256.update(first);
    sha256.update(rest);
    sha256.finalize().into()
}

pub fn tree_hash(a: &Allocator, node: NodePtr) -> [u8; 32] {
    let mut hashes = Vec::new();
    let mut ops = vec![TreeOp::SExp(node)];

    while let Some(op) = ops.pop() {
        match op {
            TreeOp::SExp(node) => match a.sexp(node) {
                SExp::Atom => {
                    hashes.push(tree_hash_atom(a.atom(node).as_ref()));
                }
                SExp::Pair(left, right) => {
                    ops.push(TreeOp::Cons);
                    ops.push(TreeOp::SExp(left));
                    ops.push(TreeOp::SExp(right));
                }
            },
            TreeOp::Cons => {
                let first = hashes.pop().unwrap();
                let rest = hashes.pop().unwrap();
                hashes.push(tree_hash_pair(first, rest));
            }
            _ => unreachable!(),
        }
    }

    assert!(hashes.len() == 1);
    hashes[0]
}

pub fn tree_hash_cached(
    a: &Allocator,
    node: NodePtr,
    backrefs: &HashSet<NodePtr>,
    cache: &mut HashMap<NodePtr, [u8; 32]>,
) -> [u8; 32] {
    let mut hashes = Vec::new();
    let mut ops = vec![TreeOp::SExp(node)];

    while let Some(op) = ops.pop() {
        match op {
            TreeOp::SExp(node) => match a.sexp(node) {
                SExp::Atom => {
                    let hash = tree_hash_atom(a.atom(node).as_ref());
                    if backrefs.contains(&node) {
                        cache.insert(node, hash);
                    }
                    hashes.push(hash);
                }
                SExp::Pair(left, right) => {
                    if let Some(hash) = cache.get(&node) {
                        hashes.push(*hash);
                    } else {
                        if backrefs.contains(&node) {
                            ops.push(TreeOp::ConsAddCache(node));
                        } else {
                            ops.push(TreeOp::Cons);
                        }
                        ops.push(TreeOp::SExp(left));
                        ops.push(TreeOp::SExp(right));
                    }
                }
            },
            TreeOp::Cons => {
                let first = hashes.pop().unwrap();
                let rest = hashes.pop().unwrap();
                hashes.push(tree_hash_pair(first, rest));
            }
            TreeOp::ConsAddCache(original_node) => {
                let first = hashes.pop().unwrap();
                let rest = hashes.pop().unwrap();
                let hash = tree_hash_pair(first, rest);
                hashes.push(hash);
                cache.insert(original_node, hash);
            }
        }
    }

    assert!(hashes.len() == 1);
    hashes[0]
}

pub fn tree_hash_from_bytes(buf: &[u8]) -> io::Result<[u8; 32]> {
    let mut a = Allocator::new();
    let node = node_from_bytes_backrefs(&mut a, buf)?;
    Ok(tree_hash(&a, node))
}

#[test]
fn test_tree_hash() {
    let mut a = Allocator::new();
    let atom1 = a.new_atom(&[1, 2, 3]).unwrap();
    let atom2 = a.new_atom(&[4, 5, 6]).unwrap();
    let root = a.new_pair(atom1, atom2).unwrap();

    // test atom1 hash
    let atom1_hash = {
        let mut sha256 = Sha256::new();
        sha256.update([1_u8]);
        sha256.update([1, 2, 3]);
        let atom1_hash = sha256.finalize();

        assert_eq!(tree_hash(&a, atom1), atom1_hash.as_slice());
        atom1_hash
    };

    // test atom2 hash
    let atom2_hash = {
        let mut sha256 = Sha256::new();
        sha256.update([1_u8]);
        sha256.update([4, 5, 6]);
        let atom2_hash = sha256.finalize();

        assert_eq!(tree_hash(&a, atom2), atom2_hash.as_slice());
        atom2_hash
    };

    // test tree hash
    let root_hash = {
        let mut sha256 = Sha256::new();
        sha256.update([2_u8]);
        sha256.update(atom1_hash.as_slice());
        sha256.update(atom2_hash.as_slice());
        let root_hash = sha256.finalize();

        assert_eq!(tree_hash(&a, root), root_hash.as_slice());
        root_hash
    };

    let atom3 = a.new_atom(&[7, 8, 9]).unwrap();
    let root2 = a.new_pair(root, atom3).unwrap();

    let atom3_hash = {
        let mut sha256 = Sha256::new();
        sha256.update([1_u8]);
        sha256.update([7, 8, 9]);
        sha256.finalize()
    };

    // test deeper tree hash
    {
        let mut sha256 = Sha256::new();
        sha256.update([2_u8]);
        sha256.update(root_hash.as_slice());
        sha256.update(atom3_hash.as_slice());

        assert_eq!(tree_hash(&a, root2), sha256.finalize().as_slice());
    }
}

#[test]
fn test_tree_hash_from_bytes() {
    use clvmr::serde::{node_to_bytes, node_to_bytes_backrefs};

    let mut a = Allocator::new();
    let atom1 = a.new_atom(&[1, 2, 3]).unwrap();
    let atom2 = a.new_atom(&[4, 5, 6]).unwrap();
    let node1 = a.new_pair(atom1, atom2).unwrap();
    let node2 = a.new_pair(atom2, atom1).unwrap();

    let node1 = a.new_pair(node1, node1).unwrap();
    let node2 = a.new_pair(node2, node2).unwrap();

    let root = a.new_pair(node1, node2).unwrap();

    let serialized_clvm = node_to_bytes(&a, root).expect("node_to_bytes");
    let serialized_clvm_backrefs =
        node_to_bytes_backrefs(&a, root).expect("node_to_bytes_backrefs");

    let hash1 = tree_hash_from_bytes(&serialized_clvm).expect("tree_hash_from_bytes");
    let hash2 = tree_hash_from_bytes(&serialized_clvm_backrefs).expect("tree_hash_from_bytes");
    let hash3 = tree_hash(&a, root);

    assert!(serialized_clvm.len() > serialized_clvm_backrefs.len());
    assert_eq!(hash1, hash2);
    assert_eq!(hash1, hash3);
}

#[cfg(test)]
use rstest::rstest;

#[cfg(test)]
#[rstest]
#[case("block-1ee588dc")]
#[case("block-6fe59b24")]
#[case("block-b45268ac")]
#[case("block-c2a8df0d")]
#[case("block-e5002df2")]
#[case("block-4671894")]
#[case("block-225758")]
#[case("block-834752")]
#[case("block-834752-compressed")]
#[case("block-834760")]
#[case("block-834761")]
#[case("block-834765")]
#[case("block-834766")]
#[case("block-834768")]
fn test_tree_hash_cached(#[case] name: &str, #[values(true, false)] compressed: bool) {
    use clvmr::serde::{
        node_from_bytes_backrefs, node_from_bytes_backrefs_record, node_to_bytes_backrefs,
    };
    use std::fs::read_to_string;

    let filename = format!("../generator-tests/{name}.txt");
    println!("file: {filename}",);
    let test_file = read_to_string(filename).expect("test file not found");
    let (generator, _) = test_file.split_once('\n').expect("invalid test file");
    let generator = hex::decode(generator).expect("invalid hex encoded generator");

    let generator = if compressed {
        let mut a = Allocator::new();
        let node = node_from_bytes_backrefs(&mut a, &generator).expect("node_from_bytes_backrefs");
        node_to_bytes_backrefs(&a, node).expect("node_to_bytes_backrefs")
    } else {
        generator
    };

    let mut a = Allocator::new();
    let mut cache = HashMap::<NodePtr, [u8; 32]>::new();
    let (node, backrefs) = node_from_bytes_backrefs_record(&mut a, &generator)
        .expect("node_from_bytes_backrefs_records");

    let hash1 = tree_hash(&a, node);
    let hash2 = tree_hash_cached(&a, node, &backrefs, &mut cache);
    // for (key, value) in cache.iter() {
    //     println!("  {key:?}: {}", hex::encode(value));
    // }
    assert_eq!(hash1, hash2);
    assert!(!compressed || !backrefs.is_empty());
}
