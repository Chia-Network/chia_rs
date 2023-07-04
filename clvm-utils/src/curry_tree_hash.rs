use sha2::{digest::FixedOutput, Digest, Sha256};

fn hash_pair(first: &[u8; 32], rest: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([2]);
    hasher.update(first);
    hasher.update(rest);
    hasher.finalize_fixed().into()
}

fn hash_atom(value: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([1]);
    hasher.update(value);
    hasher.finalize_fixed().into()
}

pub fn curry_tree_hash(program_hash: &[u8; 32], arg_hashes: &[&[u8; 32]]) -> [u8; 32] {
    let nil = hash_atom(&[]);
    let one = hash_atom(&[1]);
    let op_q = one;
    let op_a = hash_atom(&[2]);
    let op_c = hash_atom(&[4]);

    let quoted_program = hash_pair(&op_q, program_hash);
    let mut quoted_args = hash_atom(&[1]);

    for arg_hash in arg_hashes.iter().rev() {
        let quoted_arg = hash_pair(&op_q, arg_hash);
        let terminated_args = hash_pair(&quoted_args, &nil);
        let terminated_args = hash_pair(&quoted_arg, &terminated_args);
        quoted_args = hash_pair(&op_c, &terminated_args);
    }

    let terminated_args = hash_pair(&quoted_args, &nil);
    let program_and_args = hash_pair(&quoted_program, &terminated_args);
    hash_pair(&op_a, &program_and_args)
}

#[cfg(test)]
mod tests {
    use clvmr::Allocator;
    use hex::ToHex;

    use crate::{curry, tree_hash};

    use super::*;

    #[test]
    fn test_equivalence() {
        let mut a = Allocator::new();

        let program = a.new_number(2.into()).unwrap();
        let arg1 = a.new_number(5.into()).unwrap();
        let arg2 = a.new_number(8.into()).unwrap();
        let curried = curry(&mut a, program, &[arg1, arg2]).unwrap();

        let tree_hash_result = tree_hash(&a, curried);

        let program_hash = tree_hash(&a, program);
        let arg1_hash = tree_hash(&a, arg1);
        let arg2_hash = tree_hash(&a, arg2);
        let curry_tree_hash_result = curry_tree_hash(&program_hash, &[&arg1_hash, &arg2_hash]);

        assert_eq!(
            tree_hash_result.encode_hex::<String>(),
            curry_tree_hash_result.encode_hex::<String>()
        );
    }
}
