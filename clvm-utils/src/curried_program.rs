use clvm_traits::{
    clvm_list, clvm_quote, destructure_list, destructure_quote, match_list, match_quote, ClvmValue,
    FromClvm, FromClvmError, MatchByte, ToClvm, ToClvmError,
};

#[derive(Debug, Clone)]
pub struct CurriedProgram<P, A> {
    pub program: P,
    pub args: A,
}

impl<Node, P, A> FromClvm<Node> for CurriedProgram<P, A>
where
    Node: Clone,
    P: FromClvm<Node>,
    A: FromClvm<Node>,
{
    fn from_clvm<'a>(
        f: &mut impl FnMut(&Node) -> ClvmValue<'a, Node>,
        ptr: Node,
    ) -> Result<Self, FromClvmError> {
        let destructure_list!(_, destructure_quote!(program), args) =
            <match_list!(MatchByte<2>, match_quote!(P), A)>::from_clvm(f, ptr)?;

        Ok(Self { program, args })
    }
}

impl<Node, P, A> ToClvm<Node> for CurriedProgram<P, A>
where
    Node: Clone,
    P: ToClvm<Node>,
    A: ToClvm<Node>,
{
    fn to_clvm(
        &self,
        f: &mut impl FnMut(ClvmValue<Node>) -> Result<Node, ToClvmError>,
    ) -> Result<Node, ToClvmError> {
        clvm_list!(2, clvm_quote!(&self.program), &self.args).to_clvm(f)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use clvm_traits::{clvm_curried_args, FromPtr, ToPtr};
    use clvmr::{serde::node_to_bytes, Allocator};

    use super::*;

    fn check<P, A>(program: P, args: A, expected: &str)
    where
        P: Debug + Clone + PartialEq + ToPtr + FromPtr,
        A: Debug + Clone + PartialEq + ToPtr + FromPtr,
    {
        let a = &mut Allocator::new();

        let curry = CurriedProgram {
            program: program.clone(),
            args: args.clone(),
        }
        .to_ptr(a)
        .unwrap();
        let actual = node_to_bytes(a, curry).unwrap();
        assert_eq!(hex::encode(actual), expected);

        let curried: CurriedProgram<P, A> = FromPtr::from_ptr(a, curry).unwrap();
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
