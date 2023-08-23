use clvm_traits::{
    clvm_list, clvm_quote, destructure_list, destructure_quote, match_list, match_quote, FromClvm,
    MatchByte, Result, ToClvm,
};
use clvmr::{allocator::NodePtr, Allocator};

#[derive(Debug, Clone)]
pub struct CurriedProgram<T> {
    pub program: NodePtr,
    pub args: T,
}

impl<T> FromClvm for CurriedProgram<T>
where
    T: FromClvm,
{
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
        let destructure_list!(_, destructure_quote!(program), args) =
            <match_list!(MatchByte<2>, match_quote!(NodePtr), T)>::from_clvm(a, ptr)?;

        Ok(Self { program, args })
    }
}

impl<T> ToClvm for CurriedProgram<T>
where
    T: ToClvm,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        clvm_list!(2, clvm_quote!(self.program), self.args.to_clvm(a)?).to_clvm(a)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use clvm_traits::clvm_curried_args;
    use clvmr::serde::node_to_bytes;

    use super::*;

    fn check<T, A>(program: T, args: A, expected: &str)
    where
        T: Debug + ToClvm + PartialEq + FromClvm,
        A: Debug + Clone + PartialEq + ToClvm + FromClvm,
    {
        let a = &mut Allocator::new();

        let curry = CurriedProgram {
            program: program.to_clvm(a).unwrap(),
            args: args.clone(),
        }
        .to_clvm(a)
        .unwrap();
        let actual = node_to_bytes(a, curry).unwrap();
        assert_eq!(hex::encode(actual), expected);

        let curried = CurriedProgram::<A>::from_clvm(a, curry).unwrap();
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
