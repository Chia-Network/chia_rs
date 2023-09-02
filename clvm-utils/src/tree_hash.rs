use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::cost::Cost;
use clvmr::sha2::{Digest, Sha256};

enum TreeOp {
    SExp(NodePtr),
    Cons,
}

/*
    op_cost: 1
    op_cost: 1
    op_cost: 1
    traverse: 44
    traverse: {traverse_cost}
    cons_cost: 50
    traverse: 48
    cons_cost: 50
    traverse: {traverse_cost2}
    apply_cost: 90
    op_cost: 1
    traverse: 44
    op_cost: 1
    quote_cost: 20
    quote_cost: 20
    op_cost: 1
    traverse: 52
    listp_cost: 19
    if_cost: 33
    apply_cost: 90
    op_cost: 1
*/
const HASH_TRAVERSE_COST: Cost = 567;

fn subtract_cost(cost_left: &mut Cost, subtract: Cost) -> Option<()> {
    if subtract > *cost_left {
        None
    } else {
        *cost_left -= subtract;
        Some(())
    }
}

// TODO: simulate object allocation counter, to correctly implement
// LIMIT_OBJECTS restriction

// TODO: simulate CLVM stack depth to correctly implement the LIMIT_STACK
// restriction

// The traverse cost seem to depend on the environment of the caller. The base
// cost is 40 and then 4 for every bit we parse.
// When calling sha256tree directly from the `mod`, it's 48.
// When calling it from the ROM generator, it's 60
pub fn tree_hash_with_cost(
    a: &Allocator,
    node: NodePtr,
    traverse_cost2: Cost,
    cost_left: &mut Cost,
) -> Option<[u8; 32]> {
    let mut hashes: Vec<[u8; 32]> = vec![];
    let mut ops = vec![TreeOp::SExp(node)];

    let mut traverse_cost = 60;

    const SHA256_BASE_COST: Cost = 87;
    const SHA256_COST_PER_ARG: Cost = 134;
    const SHA256_COST_PER_BYTE: Cost = 2;
    const MALLOC_COST_PER_BYTE: Cost = 10;

    while let Some(op) = ops.pop() {
        match op {
            TreeOp::SExp(node) => {
                subtract_cost(
                    cost_left,
                    HASH_TRAVERSE_COST + traverse_cost + traverse_cost2,
                )?;
                traverse_cost = 56;

                match a.sexp(node) {
                    SExp::Atom => {
                        // traverse: 52
                        // quote_cost: 20
                        subtract_cost(cost_left, 72)?;
                        let mut sha256 = Sha256::new();
                        sha256.update([1_u8]);
                        let buf = a.atom(node);
                        sha256.update(buf);
                        hashes.push(sha256.finalize().into());
                        let mut sha_cost = SHA256_BASE_COST;
                        sha_cost += 2 * SHA256_COST_PER_ARG;
                        sha_cost += (1 + buf.len() as Cost) * SHA256_COST_PER_BYTE;
                        // sha256_cost: ...
                        // malloc_cost: 320
                        subtract_cost(cost_left, sha_cost + 32 * MALLOC_COST_PER_BYTE)?;
                    }
                    SExp::Pair(left, right) => {
                        ops.push(TreeOp::Cons);
                        ops.push(TreeOp::SExp(left));
                        ops.push(TreeOp::SExp(right));
                    }
                }
            }
            TreeOp::Cons => {
                // quote_cost: 20
                subtract_cost(cost_left, 20)?;

                let mut sha256 = Sha256::new();
                sha256.update([2_u8]);
                sha256.update(hashes.pop().unwrap());
                sha256.update(hashes.pop().unwrap());
                hashes.push(sha256.finalize().into());

                const SHA_COST: Cost = SHA256_BASE_COST;
                // sha256_cost: 619
                // malloc_cost: 320
                subtract_cost(
                    cost_left,
                    3 * SHA256_COST_PER_ARG
                        + (1 + 32 + 32) * SHA256_COST_PER_BYTE
                        + SHA_COST
                        + 32 * MALLOC_COST_PER_BYTE,
                )?;
            }
        }
    }

    assert!(hashes.len() == 1);
    Some(hashes[0])
}

#[cfg(test)]
pub fn cmp_hash(a: &mut Allocator, root: NodePtr) {
    use clvmr::chia_dialect::ChiaDialect;
    use clvmr::reduction::Reduction;
    use clvmr::run_program::run_program;
    use clvmr::serde::node_from_bytes;

    /*
        This is the compiled version of:
        (mod (TREE)
            (defun sha256tree (TREE)
                (if (l TREE)
                    (sha256 2 (sha256tree (f TREE)) (sha256tree (r TREE)))
                    (sha256 1 TREE)
                )
            )
            (sha256tree TREE)
        )

        CLVM:

        (a (q 2 2 (c 2 (c 5 ())))
            (c (q 2 (i (l 5)
                (q #sha256 (q . 2)
                    (a 2 (c 2 (c 9 ())))
                    (a 2 (c 2 (c 13 ())))
                )
                (q #sha256 (q . 1) 5)
            ) 1) 1)
        )
    */
    let tree_hash_clvm: Vec<u8> = hex::decode(
        "\
ff02ffff01ff02ff02ffff04ff02ffff04ff05ff80808080ffff04ffff01ff02\
ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff02ffff04ff02ffff04ff\
09ff80808080ffff02ff02ffff04ff02ffff04ff0dff8080808080ffff01ff0b\
ffff0101ff058080ff0180ff018080",
    )
    .expect("hex::decode");

    let program = node_from_bytes(a, &tree_hash_clvm).expect("node_from_bytes");

    let argument = a.new_pair(root, a.null()).unwrap();

    let dialect = ChiaDialect::new(0);
    let Reduction(cost1, ret1) =
        run_program(a, &dialect, program, argument, 11000000000).expect("run_program");

    // 226 is the overhead of starting the program and calling into the mod
    // When running it directly in the mod scope, the first iteration
    // environment lookups (path traversal) needs 2 fewer bits, and so is 8 cost
    // cheaper. To make the cost match, we therefore deduct 8
    let mut expect_cost = cost1 - 226 + 8;
    let ret2 = tree_hash_with_cost(a, root, 48, &mut expect_cost).unwrap();

    assert_eq!(a.atom(ret1), ret2);
    println!("clvm cost: {cost1} rust cost: {}", cost1 - expect_cost);
    assert_eq!(expect_cost, 0);
}

#[test]
fn test_tree_hash_cost() {
    use clvmr::Allocator;

    let mut a = Allocator::new();

    let atom3 = a.new_atom(&[1, 2, 3]).unwrap();
    let atom2 = a.new_atom(&[4, 5]).unwrap();
    let atom1 = a.new_atom(&[6]).unwrap();
    let atom_c = a.new_atom(&[0xcc, 0xcc]).unwrap();
    let atom_a = a.new_atom(&[0xaa, 0xaa]).unwrap();

    cmp_hash(&mut a, atom3);

    let root1 = a.new_pair(atom1, atom1).unwrap();
    let root2 = a.new_pair(atom1, atom1).unwrap();
    let root = a.new_pair(root1, root2).unwrap();
    cmp_hash(&mut a, root);

    let root = a.new_pair(atom1, atom2).unwrap();
    cmp_hash(&mut a, root);

    let root = a.new_pair(atom2, atom3).unwrap();
    cmp_hash(&mut a, root);

    let root = a.new_pair(atom1, atom3).unwrap();
    cmp_hash(&mut a, root);

    let root = a.new_pair(atom2, root).unwrap();
    cmp_hash(&mut a, root);

    let root = a.new_pair(atom_c, atom_a).unwrap();
    cmp_hash(&mut a, root);
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
                hashes.push(tree_hash_pair(first, rest));
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
