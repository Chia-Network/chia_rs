use crate::{tree_hash_atom, tree_hash_pair, TreeHash};

pub fn curry_tree_hash(program_hash: TreeHash, arg_hashes: &[TreeHash]) -> TreeHash {
    let nil = tree_hash_atom(&[]);
    let op_q = tree_hash_atom(&[1]);
    let op_a = tree_hash_atom(&[2]);
    let op_c = tree_hash_atom(&[4]);

    let quoted_program = tree_hash_pair(op_q, program_hash);
    let mut quoted_args = tree_hash_atom(&[1]);

    for &arg_hash in arg_hashes.iter().rev() {
        let quoted_arg = tree_hash_pair(op_q, arg_hash);
        let terminated_args = tree_hash_pair(quoted_args, nil);
        let terminated_args = tree_hash_pair(quoted_arg, terminated_args);
        quoted_args = tree_hash_pair(op_c, terminated_args);
    }

    let terminated_args = tree_hash_pair(quoted_args, nil);
    let program_and_args = tree_hash_pair(quoted_program, terminated_args);
    tree_hash_pair(op_a, program_and_args)
}

#[cfg(test)]
mod tests {
    use clvm_traits::{clvm_curried_args, ToClvm};
    use clvmr::Allocator;
    use hex::ToHex;

    use crate::{tree_hash, CurriedProgram};

    use super::*;

    #[test]
    fn test_equivalence() {
        let mut a = Allocator::new();

        let program = a.new_number(2.into()).unwrap();
        let arg_1 = a.new_number(5.into()).unwrap();
        let arg_2 = a.new_number(8.into()).unwrap();
        let args = clvm_curried_args!(5, 8).to_clvm(&mut a).unwrap();
        let curried = CurriedProgram { program, args }.to_clvm(&mut a).unwrap();

        let tree_hash_result = tree_hash(&a, curried);

        let program_hash = tree_hash(&a, program);
        let arg_1_hash = tree_hash(&a, arg_1);
        let arg_2_hash = tree_hash(&a, arg_2);
        let curry_tree_hash_result = curry_tree_hash(program_hash, &[arg_1_hash, arg_2_hash]);

        assert_eq!(
            tree_hash_result.encode_hex::<String>(),
            curry_tree_hash_result.encode_hex::<String>()
        );
    }
}
