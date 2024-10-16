use chia_sha2::Sha256;
use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::serde::node_from_bytes_backrefs_record;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::{fmt, io};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeHash([u8; 32]);

impl TreeHash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    pub const fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl fmt::Debug for TreeHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TreeHash({self})")
    }
}

impl fmt::Display for TreeHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl From<[u8; 32]> for TreeHash {
    fn from(hash: [u8; 32]) -> Self {
        Self::new(hash)
    }
}

impl From<TreeHash> for [u8; 32] {
    fn from(hash: TreeHash) -> [u8; 32] {
        hash.0
    }
}

impl AsRef<[u8]> for TreeHash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for TreeHash {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

enum TreeOp {
    SExp(NodePtr),
    Cons,
    ConsAddCache(NodePtr),
}

pub fn tree_hash_atom(bytes: &[u8]) -> TreeHash {
    let mut sha256 = Sha256::new();
    sha256.update([1]);
    sha256.update(bytes);
    TreeHash::new(sha256.finalize())
}

pub fn tree_hash_pair(first: TreeHash, rest: TreeHash) -> TreeHash {
    let mut sha256 = Sha256::new();
    sha256.update([2]);
    sha256.update(first);
    sha256.update(rest);
    TreeHash::new(sha256.finalize())
}

pub fn tree_hash(a: &Allocator, node: NodePtr) -> TreeHash {
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
            TreeOp::ConsAddCache(_) => unreachable!(),
        }
    }

    assert!(hashes.len() == 1);
    hashes[0]
}

pub fn tree_hash_cached<S>(
    a: &Allocator,
    node: NodePtr,
    backrefs: &HashSet<NodePtr, S>,
    cache: &mut HashMap<NodePtr, TreeHash, S>,
) -> TreeHash
where
    S: std::hash::BuildHasher,
{
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

pub fn tree_hash_from_bytes(buf: &[u8]) -> io::Result<TreeHash> {
    let mut a = Allocator::new();
    let (node, backrefs) = node_from_bytes_backrefs_record(&mut a, buf)?;
    let mut cache = HashMap::<NodePtr, TreeHash>::new();
    Ok(tree_hash_cached(&a, node, &backrefs, &mut cache))
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

        assert_eq!(tree_hash(&a, atom1).as_ref(), atom1_hash.as_slice());
        atom1_hash
    };

    // test atom2 hash
    let atom2_hash = {
        let mut sha256 = Sha256::new();
        sha256.update([1_u8]);
        sha256.update([4, 5, 6]);
        let atom2_hash = sha256.finalize();

        assert_eq!(tree_hash(&a, atom2).as_ref(), atom2_hash.as_slice());
        atom2_hash
    };

    // test tree hash
    let root_hash = {
        let mut sha256 = Sha256::new();
        sha256.update([2_u8]);
        sha256.update(atom1_hash.as_slice());
        sha256.update(atom2_hash.as_slice());
        let root_hash = sha256.finalize();

        assert_eq!(tree_hash(&a, root).as_ref(), root_hash.as_slice());
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

        assert_eq!(tree_hash(&a, root2).as_ref(), sha256.finalize().as_slice());
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
#[case(
    "block-1ee588dc",
    "1cba0b22b84b597d265d77fbabb57fada01d963f75dc3956a6166a2385997ef2"
)]
#[case(
    "block-6fe59b24",
    "540c5afac7c26728ed6b7891d8ce2f5b26009c4b0090d7035403c2425dc54e1d"
)]
#[case(
    "block-b45268ac",
    "7cc321f5554126c9f430afbc7dd9c804f5d34a248e3192f275f5d585ecf8e873"
)]
#[case(
    "block-c2a8df0d",
    "2e25efa524e420111006fee77f50fb8fbd725920a5312d5480af239d81ab5e7e"
)]
#[case(
    "block-e5002df2",
    "c179ece232dceef984ba000f7e5b67ee3092582668bf6178969df10845eb8b18"
)]
#[case(
    "block-4671894",
    "3750f0e1bde9fcb407135f974aa276a4580e1e76a47e6d8d9bb2911d0fe91db1"
)]
#[case(
    "block-225758",
    "880df94c3c9e0f7c26c42ae99723e683a4cd37e73f74c6322d1dfabaa1d64d93"
)]
#[case(
    "block-834752",
    "be755b8ef03d917b8bd37ae152792a7daa7de81bbb0eaa21c530571c2105c130"
)]
#[case(
    "block-834752-compressed",
    "be755b8ef03d917b8bd37ae152792a7daa7de81bbb0eaa21c530571c2105c130"
)]
#[case(
    "block-834760",
    "77558768f74c5f863b36232a1390843a63a397fc22da1321fea3a05eab67be2c"
)]
#[case(
    "block-834761",
    "4bac8b299c6545a37a825883c863b79ce850e7f6c8f1d2abeec2865f5450f1c5"
)]
#[case(
    "block-834765",
    "b915ec5f9f8ea723e0a99b035df206673369b802766dd76b6c8f4c15ab7bca2c"
)]
#[case(
    "block-834766",
    "409559c3395fb18a6c3390ccccd55e82162b1e68b867490a90ccbddf78147c9d"
)]
#[case(
    "block-834768",
    "905441945a9a56558337c8b7a536a6b9606ad63e11a265a938f301747ccfb7af"
)]
fn test_tree_hash_cached(
    #[case] name: &str,
    #[case] expect: &str,
    #[values(true, false)] compressed: bool,
) {
    use clvmr::serde::{
        node_from_bytes_backrefs, node_from_bytes_backrefs_record, node_to_bytes_backrefs,
    };
    use std::fs::read_to_string;

    let filename = format!("../../generator-tests/{name}.txt");
    println!("file: {filename}",);
    let test_file = read_to_string(filename).expect("test file not found");
    let generator = test_file.lines().next().expect("invalid test file");
    let generator = hex::decode(generator).expect("invalid hex encoded generator");

    let generator = if compressed {
        let mut a = Allocator::new();
        let node = node_from_bytes_backrefs(&mut a, &generator).expect("node_from_bytes_backrefs");
        node_to_bytes_backrefs(&a, node).expect("node_to_bytes_backrefs")
    } else {
        generator
    };

    let mut a = Allocator::new();
    let mut cache = HashMap::<NodePtr, TreeHash>::new();
    let (node, backrefs) = node_from_bytes_backrefs_record(&mut a, &generator)
        .expect("node_from_bytes_backrefs_records");

    let hash1 = tree_hash(&a, node);
    let hash2 = tree_hash_cached(&a, node, &backrefs, &mut cache);
    // for (key, value) in cache.iter() {
    //     println!("  {key:?}: {}", hex::encode(value));
    // }
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.as_ref(), hex::decode(expect).unwrap().as_slice());
    assert!(!compressed || !backrefs.is_empty());
}

#[cfg(test)]
fn test_sha256_atom(buf: &[u8]) {
    let hash = tree_hash_atom(buf);

    let mut hasher = Sha256::new();
    hasher.update([1_u8]);
    if !buf.is_empty() {
        hasher.update(buf);
    }

    assert_eq!(hash.as_ref(), hasher.finalize().as_slice());
}

#[test]
fn test_tree_hash_atom() {
    test_sha256_atom(&[]);
    for val in 0..255 {
        test_sha256_atom(&[val]);
    }

    for val in 0..255 {
        test_sha256_atom(&[0, val]);
    }

    for val in 0..255 {
        test_sha256_atom(&[0xff, val]);
    }
}
