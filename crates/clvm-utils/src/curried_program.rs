use clvm_traits::{
    clvm_list, clvm_quote, destructure_list, destructure_quote, match_list, match_quote,
    ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, MatchByte, ToClvm, ToClvmError,
};

#[derive(Debug, Clone)]
pub struct CurriedProgram<P, A> {
    pub program: P,
    pub args: A,
}

impl<N, P, A> FromClvm<N> for CurriedProgram<P, A>
where
    P: FromClvm<N>,
    A: FromClvm<N>,
{
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        let destructure_list!(_, destructure_quote!(program), args) =
            <match_list!(MatchByte<2>, match_quote!(P), A)>::from_clvm(decoder, node)?;
        Ok(Self { program, args })
    }
}

impl<N, P, A> ToClvm<N> for CurriedProgram<P, A>
where
    P: ToClvm<N>,
    A: ToClvm<N>,
{
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        clvm_list!(2, clvm_quote!(&self.program), &self.args).to_clvm(encoder)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use clvm_traits::clvm_curried_args;
    use clvmr::{serde::node_to_bytes, Allocator, NodePtr};

    use super::*;

    fn check<P, A>(program: P, args: A, expected: &str)
    where
        P: Debug + PartialEq + ToClvm<NodePtr> + FromClvm<NodePtr>,
        A: Debug + PartialEq + ToClvm<NodePtr> + FromClvm<NodePtr>,
    {
        let a = &mut Allocator::new();

        let curry = CurriedProgram {
            program: &program,
            args: &args,
        }
        .to_clvm(a)
        .unwrap();
        let actual = node_to_bytes(a, curry).unwrap();
        assert_eq!(hex::encode(actual), expected);

        let curried = CurriedProgram::<P, A>::from_clvm(a, curry).unwrap();
        assert_eq!(curried.program, program);
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
