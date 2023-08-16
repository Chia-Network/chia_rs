use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};

use crate::{
    clvm_list, clvm_quote, destructure_list, destructure_quote, match_list, match_quote, Error,
    FromClvm, MatchByte, Result, ToClvm,
};

#[derive(Debug, Clone)]
pub struct CurriedArgs(pub Vec<NodePtr>);

impl FromClvm for CurriedArgs {
    fn from_clvm(a: &Allocator, mut ptr: NodePtr) -> Result<Self> {
        let mut items = Vec::new();
        loop {
            match a.sexp(ptr) {
                SExp::Atom => {
                    if ptr == a.one() {
                        return Ok(Self(items));
                    } else {
                        return Err(Error::ExpectedOne(ptr));
                    }
                }
                SExp::Pair(..) => {
                    let destructure_list!(_, destructure_quote!(first), rest) =
                        <match_list!(MatchByte<4>, match_quote!(NodePtr), NodePtr)>::from_clvm(
                            a, ptr,
                        )?;

                    items.push(first);
                    ptr = rest;
                }
            }
        }
    }
}

impl ToClvm for CurriedArgs {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let mut result = a.one();
        for item in self.0.iter().rev() {
            result = clvm_list!(4, clvm_quote!(item), result).to_clvm(a)?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use clvmr::serde::node_to_bytes;

    use super::*;

    fn check(a: &mut Allocator, args: CurriedArgs, expected: &str) {
        let ptr = args.to_clvm(a).unwrap();
        let actual = node_to_bytes(a, ptr).unwrap();
        assert_eq!(hex::encode(actual), expected);

        let roundtrip = CurriedArgs::from_clvm(a, ptr).unwrap();

        for (i, arg) in roundtrip.0.iter().enumerate() {
            let actual = node_to_bytes(a, *arg).unwrap();
            let expected = node_to_bytes(a, args.0[i]).unwrap();
            assert_eq!(hex::encode(actual), hex::encode(expected));
        }
    }

    #[test]
    fn curried_args() {
        let a = &mut Allocator::new();

        let args = CurriedArgs(vec![
            "a".to_clvm(a).unwrap(),
            "b".to_clvm(a).unwrap(),
            "c".to_clvm(a).unwrap(),
        ]);

        check(
            a,
            args,
            "ff04ffff0161ffff04ffff0162ffff04ffff0163ff01808080",
        );
    }
}
