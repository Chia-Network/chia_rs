use clvmr::allocator::{Allocator, NodePtr, SExp};
use sha2::{digest::FixedOutput, Digest, Sha256};

enum TreeOp {
    SExp(NodePtr),
    Cons,
}

pub fn tree_hash(a: &Allocator, node: NodePtr) -> [u8; 32] {
    let mut hashes: Vec<[u8; 32]> = vec![];
    let mut ops = vec![TreeOp::SExp(node)];

    while let Some(op) = ops.pop() {
        match op {
            TreeOp::SExp(node) => match a.sexp(node) {
                SExp::Atom() => {
                    hashes.push(tree_hash_atom(a.atom(node)));
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
                hashes.push(tree_hash_pair(&first, &rest));
            }
        }
    }

    assert!(hashes.len() == 1);
    hashes[0]
}

pub fn tree_hash_pair(first: &[u8; 32], rest: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([2]);
    hasher.update(first);
    hasher.update(rest);
    hasher.finalize_fixed().into()
}

pub fn tree_hash_atom(value: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([1]);
    hasher.update(value);
    hasher.finalize_fixed().into()
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
        sha256.update(&[1_u8]);
        sha256.update(&[1, 2, 3]);
        let atom1_hash = sha256.finalize();

        assert_eq!(tree_hash(&a, atom1), atom1_hash.as_slice());
        atom1_hash
    };

    // test atom2 hash
    let atom2_hash = {
        let mut sha256 = Sha256::new();
        sha256.update(&[1_u8]);
        sha256.update(&[4, 5, 6]);
        let atom2_hash = sha256.finalize();

        assert_eq!(tree_hash(&a, atom2), atom2_hash.as_slice());
        atom2_hash
    };

    // test tree hash
    let root_hash = {
        let mut sha256 = Sha256::new();
        sha256.update(&[2_u8]);
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
        sha256.update(&[1_u8]);
        sha256.update(&[7, 8, 9]);
        sha256.finalize()
    };

    // test deeper tree hash
    {
        let mut sha256 = Sha256::new();
        sha256.update(&[2_u8]);
        sha256.update(root_hash.as_slice());
        sha256.update(atom3_hash.as_slice());

        assert_eq!(tree_hash(&a, root2), sha256.finalize().as_slice());
    }
}
