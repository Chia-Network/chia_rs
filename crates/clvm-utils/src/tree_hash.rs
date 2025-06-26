use chia_sha2::Sha256;
use clvmr::allocator::{Allocator, NodePtr, NodeVisitor, ObjectType, SExp};
use clvmr::serde::node_from_bytes_backrefs;
use hex_literal::hex;
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

#[derive(Default)]
pub struct TreeCache {
    hashes: Vec<TreeHash>,
    // each entry is an index into hashes, or one of 3 special values:
    // u16::MAX if the pair has not been visited
    // u16::MAX - 1 if the pair has been seen once
    // u16::MAX - 2 if the pair has been seen at least twice (this makes it a
    // candidate for memoization)
    pairs: Vec<u16>,
}

const NOT_VISITED: u16 = u16::MAX;
const SEEN_ONCE: u16 = u16::MAX - 1;
const SEEN_MULTIPLE: u16 = u16::MAX - 2;

impl TreeCache {
    pub fn get(&self, n: NodePtr) -> Option<&TreeHash> {
        // We only cache pairs (for now)
        if !matches!(n.object_type(), ObjectType::Pair) {
            return None;
        }

        let idx = n.index() as usize;
        let slot = *self.pairs.get(idx)?;
        if slot >= SEEN_MULTIPLE {
            return None;
        }
        Some(&self.hashes[slot as usize])
    }

    pub fn insert(&mut self, n: NodePtr, hash: &TreeHash) {
        // If we've reached the max size, just ignore new cache items
        if self.hashes.len() == SEEN_MULTIPLE as usize {
            return;
        }

        if !matches!(n.object_type(), ObjectType::Pair) {
            return;
        }

        let idx = n.index() as usize;
        if idx >= self.pairs.len() {
            self.pairs.resize(idx + 1, NOT_VISITED);
        }

        let slot = self.hashes.len();
        self.hashes.push(*hash);
        self.pairs[idx] = slot as u16;
    }

    /// mark the node as being visited. Returns true if we need to
    /// traverse visitation down this node.
    fn visit(&mut self, n: NodePtr) -> bool {
        if !matches!(n.object_type(), ObjectType::Pair) {
            return false;
        }
        let idx = n.index() as usize;
        if idx >= self.pairs.len() {
            self.pairs.resize(idx + 1, NOT_VISITED);
        }
        if self.pairs[idx] > SEEN_MULTIPLE {
            self.pairs[idx] -= 1;
        }
        self.pairs[idx] == SEEN_ONCE
    }

    pub fn should_memoize(&mut self, n: NodePtr) -> bool {
        if !matches!(n.object_type(), ObjectType::Pair) {
            return false;
        }
        let idx = n.index() as usize;
        if idx >= self.pairs.len() {
            false
        } else {
            self.pairs[idx] <= SEEN_MULTIPLE
        }
    }

    pub fn visit_tree(&mut self, a: &Allocator, node: NodePtr) {
        if !self.visit(node) {
            return;
        }
        let mut nodes = vec![node];
        while let Some(n) = nodes.pop() {
            let SExp::Pair(left, right) = a.sexp(n) else {
                continue;
            };
            if self.visit(left) {
                nodes.push(left);
            }
            if self.visit(right) {
                nodes.push(right);
            }
        }
    }
}

enum TreeOp {
    SExp(NodePtr),
    Cons,
    ConsAddCache(NodePtr),
}

// contains SHA256(1 .. x), where x is the index into the array and .. is
// concatenation. This was computed by:
// from hashlib import sha256
// print(f"    th!(\"{sha256(bytes([1])).hexdigest()}\"),")
// for i in range(1, 24):
//     print(f"    th!(\"{sha256(bytes([1, i])).hexdigest()}\"),")

macro_rules! th {
    ($hash:expr) => {
        TreeHash::new(hex!($hash))
    };
}
pub const PRECOMPUTED_HASHES: [TreeHash; 24] = [
    th!("4bf5122f344554c53bde2ebb8cd2b7e3d1600ad631c385a5d7cce23c7785459a"),
    th!("9dcf97a184f32623d11a73124ceb99a5709b083721e878a16d78f596718ba7b2"),
    th!("a12871fee210fb8619291eaea194581cbd2531e4b23759d225f6806923f63222"),
    th!("c79b932e1e1da3c0e098e5ad2c422937eb904a76cf61d83975a74a68fbb04b99"),
    th!("a8d5dd63fba471ebcb1f3e8f7c1e1879b7152a6e7298a91ce119a63400ade7c5"),
    th!("bc5959f43bc6e47175374b6716e53c9a7d72c59424c821336995bad760d9aeb3"),
    th!("44602a999abbebedf7de0ae1318e4f57e3cb1d67e482a65f9657f7541f3fe4bb"),
    th!("ca6c6588fa01171b200740344d354e8548b7470061fb32a34f4feee470ec281f"),
    th!("9e6282e4f25e370ce617e21d6fe265e88b9e7b8682cf00059b9d128d9381f09d"),
    th!("ac9e61d54eb6967e212c06aab15408292f8558c48f06f9d705150063c68753b0"),
    th!("c04b5bb1a5b2eb3e9cd4805420dba5a9d133da5b7adeeafb5474c4adae9faa80"),
    th!("57bfd1cb0adda3d94315053fda723f2028320faa8338225d99f629e3d46d43a9"),
    th!("6b6daa8334bbcc8f6b5906b6c04be041d92700b74024f73f50e0a9f0dae5f06f"),
    th!("c7b89cfb9abf2c4cb212a4840b37d762f4c880b8517b0dadb0c310ded24dd86d"),
    th!("653b3bb3e18ef84d5b1e8ff9884aecf1950c7a1c98715411c22b987663b86dda"),
    th!("24255ef5d941493b9978f3aabb0ed07d084ade196d23f463ff058954cbf6e9b6"),
    th!("af340aa58ea7d72c2f9a7405f3734167bb27dd2a520d216addef65f8362102b6"),
    th!("26e7f98cfafee5b213726e22632923bf31bf3e988233235f8f5ca5466b3ac0ed"),
    th!("115b498ce94335826baa16386cd1e2fde8ca408f6f50f3785964f263cdf37ebe"),
    th!("d8c50d6282a1ba47f0a23430d177bbfbb72e2b84713745e894f575570f1f3d6e"),
    th!("dbe726e81a7221a385e007ef9e834a975a4b528c6f55a5d2ece288bee831a3d1"),
    th!("764c8a3561c7cf261771b4e1969b84c210836f3c034baebac5e49a394a6ee0a9"),
    th!("dce37f3512b6337d27290436ba9289e2fd6c775494c33668dd177cf811fbd47a"),
    th!("5809addc9f6926fc5c4e20cf87958858c4454c21cdfc6b02f377f12c06b35cca"),
];

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
            TreeOp::SExp(node) => match a.node(node) {
                NodeVisitor::Buffer(bytes) => {
                    hashes.push(tree_hash_atom(bytes));
                }
                NodeVisitor::U32(val) => {
                    if (val as usize) < PRECOMPUTED_HASHES.len() {
                        hashes.push(PRECOMPUTED_HASHES[val as usize]);
                    } else {
                        hashes.push(tree_hash_atom(a.atom(node).as_ref()));
                    }
                }
                NodeVisitor::Pair(left, right) => {
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

pub fn tree_hash_cached(a: &Allocator, node: NodePtr, cache: &mut TreeCache) -> TreeHash {
    cache.visit_tree(a, node);

    let mut hashes = Vec::new();
    let mut ops = vec![TreeOp::SExp(node)];

    while let Some(op) = ops.pop() {
        match op {
            TreeOp::SExp(node) => match a.node(node) {
                NodeVisitor::Buffer(bytes) => {
                    let hash = tree_hash_atom(bytes);
                    hashes.push(hash);
                }
                NodeVisitor::U32(val) => {
                    if (val as usize) < PRECOMPUTED_HASHES.len() {
                        hashes.push(PRECOMPUTED_HASHES[val as usize]);
                    } else {
                        hashes.push(tree_hash_atom(a.atom(node).as_ref()));
                    }
                }
                NodeVisitor::Pair(left, right) => {
                    if let Some(hash) = cache.get(node) {
                        hashes.push(*hash);
                    } else {
                        if cache.should_memoize(node) {
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
                cache.insert(original_node, &hash);
            }
        }
    }

    assert!(hashes.len() == 1);
    hashes[0]
}

pub fn tree_hash_from_bytes(buf: &[u8]) -> io::Result<TreeHash> {
    let mut a = Allocator::new();
    let node = node_from_bytes_backrefs(&mut a, buf)?;
    let mut cache = TreeCache::default();
    Ok(tree_hash_cached(&a, node, &mut cache))
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
    use clvmr::serde::{node_from_bytes_backrefs, node_to_bytes_backrefs};
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
    let mut cache = TreeCache::default();
    let node = node_from_bytes_backrefs(&mut a, &generator).expect("node_from_bytes_backrefs");

    let hash1 = tree_hash(&a, node);
    let hash2 = tree_hash_cached(&a, node, &mut cache);
    // for (key, value) in cache.iter() {
    //     println!("  {key:?}: {}", hex::encode(value));
    // }
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.as_ref(), hex::decode(expect).unwrap().as_slice());
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
    for val in 0..=255 {
        test_sha256_atom(&[val]);
    }

    for val in 0..=255 {
        test_sha256_atom(&[0, val]);
    }

    for val in 0..=255 {
        test_sha256_atom(&[0xff, val]);
    }
}

#[test]
fn test_precomputed_atoms() {
    assert_eq!(tree_hash_atom(&[]), PRECOMPUTED_HASHES[0]);
    for val in 1..(PRECOMPUTED_HASHES.len() as u8) {
        assert_eq!(tree_hash_atom(&[val]), PRECOMPUTED_HASHES[val as usize]);
    }
}

#[test]
fn test_tree_cache_get() {
    let mut allocator = Allocator::new();
    let mut cache = TreeCache::default();

    let a = allocator.nil();
    let b = allocator.one();
    let c = allocator.new_pair(a, b).expect("new_pair");

    assert_eq!(cache.get(a), None);
    assert_eq!(cache.get(b), None);
    assert_eq!(cache.get(c), None);

    // We don't cache atoms
    cache.insert(a, &tree_hash(&allocator, a));
    assert_eq!(cache.get(a), None);

    cache.insert(b, &tree_hash(&allocator, b));
    assert_eq!(cache.get(b), None);

    // but pair is OK
    cache.insert(c, &tree_hash(&allocator, c));
    assert_eq!(cache.get(c), Some(&tree_hash(&allocator, c)));
}

#[test]
fn test_tree_cache_size_limit() {
    let mut allocator = Allocator::new();
    let mut cache = TreeCache::default();

    let mut list = allocator.nil();
    let mut hash = tree_hash(&allocator, list);
    cache.insert(list, &hash);

    // we only fit 65k items in the cache
    for i in 0..65540 {
        let b = allocator.one();
        list = allocator.new_pair(b, list).expect("new_pair");
        hash = tree_hash_pair(tree_hash_atom(b"\x01"), hash);
        cache.insert(list, &hash);

        println!("{i}");
        if i < 65533 {
            assert_eq!(cache.get(list), Some(&hash));
        } else {
            assert_eq!(cache.get(list), None);
        }
    }
    assert_eq!(cache.get(list), None);
}

#[test]
fn test_tree_cache_should_memoize() {
    let mut allocator = Allocator::new();
    let mut cache = TreeCache::default();

    let a = allocator.nil();
    let b = allocator.one();
    let c = allocator.new_pair(a, b).expect("new_pair");

    assert!(!cache.should_memoize(a));
    assert!(!cache.should_memoize(b));
    assert!(!cache.should_memoize(c));

    // we need to visit a node at least twice for it to be considered a
    // candidate for memoization
    assert!(cache.visit(c));
    assert!(!cache.should_memoize(c));
    assert!(!cache.visit(c));

    assert!(cache.should_memoize(c));
}
