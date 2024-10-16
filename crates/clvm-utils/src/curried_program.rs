use clvm_traits::{
    clvm_list, clvm_quote, destructure_list, destructure_quote, match_list, match_quote,
    ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, MatchByte, ToClvm, ToClvmError,
};

#[derive(Debug, Clone)]
pub struct CurriedProgram<P, A> {
    pub program: P,
    pub args: A,
}

impl<N, D: ClvmDecoder<Node = N>, P, A> FromClvm<D> for CurriedProgram<P, A>
where
    P: FromClvm<D>,
    A: FromClvm<D>,
{
    fn from_clvm(decoder: &D, node: N) -> Result<Self, FromClvmError> {
        let destructure_list!(_, destructure_quote!(program), args) =
            <match_list!(MatchByte<2>, match_quote!(P), A)>::from_clvm(decoder, node)?;
        Ok(Self { program, args })
    }
}

impl<N, E: ClvmEncoder<Node = N>, P, A> ToClvm<E> for CurriedProgram<P, A>
where
    P: ToClvm<E>,
    A: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        clvm_list!(2, clvm_quote!(&self.program), &self.args).to_clvm(encoder)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use clvm_traits::clvm_curried_args;
    use clvmr::{serde::node_to_bytes, Allocator};

    use super::*;

    fn check<P, A>(program: &P, args: &A, expected: &str)
    where
        P: Debug + PartialEq + ToClvm<Allocator> + FromClvm<Allocator>,
        A: Debug + PartialEq + ToClvm<Allocator> + FromClvm<Allocator>,
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
        assert_eq!(&curried.program, program);
        assert_eq!(&curried.args, args);
    }

    #[test]
    fn curry() {
        check(
            &"xyz".to_string(),
            &clvm_curried_args!("a".to_string(), "b".to_string(), "c".to_string()),
            "ff02ffff018378797affff04ffff0161ffff04ffff0162ffff04ffff0163ff0180808080",
        );
    }
}
