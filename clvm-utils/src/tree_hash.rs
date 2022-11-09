use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::sha2::{Digest, Sha256};

enum TreeOp {
    SExp(NodePtr),
    Cons,
}

pub fn tree_hash(a: &Allocator, node: NodePtr) -> [u8; 32] {
    let mut hashes: Vec<[u8; 32]> = vec![];
    let mut ops = vec![TreeOp::SExp(node)];

    while !ops.is_empty() {
        match ops.pop().unwrap() {
            TreeOp::SExp(node) => match a.sexp(node) {
                SExp::Atom(atom) => {
                    let mut sha256 = Sha256::new();
                    sha256.update([1_u8]);
                    sha256.update(a.buf(&atom));
                    hashes.push(sha256.finalize().into())
                }
                SExp::Pair(left, right) => {
                    ops.push(TreeOp::Cons);
                    ops.push(TreeOp::SExp(left));
                    ops.push(TreeOp::SExp(right));
                }
            },
            TreeOp::Cons => {
                let mut sha256 = Sha256::new();
                sha256.update([2_u8]);
                sha256.update(hashes.pop().unwrap());
                sha256.update(hashes.pop().unwrap());
                hashes.push(sha256.finalize().into());
            }
        }
    }

    assert!(hashes.len() == 1);
    hashes[0]
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
