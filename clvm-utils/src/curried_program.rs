use clvm_traits::{
    clvm_list, clvm_quote, destructure_list, destructure_quote, match_list, match_quote, BuildTree,
    FromClvm, MatchByte, Result, Value,
};
use clvmr::{allocator::NodePtr, Allocator};

#[derive(Debug, Clone)]
pub struct CurriedProgram<N, T> {
    pub program: N,
    pub args: T,
}

impl<T> FromClvm for CurriedProgram<NodePtr, T>
where
    T: FromClvm,
{
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
        let destructure_list!(_, destructure_quote!(program), args) =
            <match_list!(MatchByte<2>, match_quote!(NodePtr), T)>::from_clvm(a, ptr)?;

        Ok(Self { program, args })
    }
}

impl<N, T> BuildTree<N> for CurriedProgram<N, T>
where
    N: BuildTree<N>,
    T: BuildTree<N>,
{
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        clvm_list!(2, clvm_quote!(&self.program), self.args.build_tree(f)?).build_tree(f)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use clvm_traits::{clvm_curried_args, ToClvm};
    use clvmr::serde::node_to_bytes;

    use super::*;

    fn check<T, A>(program: T, args: A, expected: &str)
    where
        T: Debug + BuildTree<NodePtr> + PartialEq + FromClvm,
        A: Debug + PartialEq + BuildTree<NodePtr> + FromClvm,
    {
        let a = &mut Allocator::new();

        let program_ptr = program.to_clvm(a).unwrap();

        let curry = CurriedProgram {
            program: program_ptr,
            args: &args,
        }
        .to_clvm(a)
        .unwrap();
        let actual = node_to_bytes(a, curry).unwrap();
        assert_eq!(hex::encode(actual), expected);

        let curried = CurriedProgram::<_, A>::from_clvm(a, curry).unwrap();
        let round_program = T::from_clvm(a, curried.program).unwrap();
        assert_eq!(round_program, program);
        assert_eq!(curried.args, args);
    }

    #[test]
    fn curry() {
        check(
            "xyz".to_string(),
            clvm_curried_args!("a".to_string(), "b".to_string(), "c".to_string()),
            "ff02ffff018378797affff04ffff0161ffff04ffff0162ffff04ffff0163ff0180808080",
        );
    }
}
