use std::fmt;

use proc_macro2::{Ident, Span};
use syn::{
    ext::IdentExt, punctuated::Punctuated, Attribute, GenericParam, Generics, Token, TypeParamBound,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Repr {
    Tuple,
    List,
    Curry,
}

impl fmt::Display for Repr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Tuple => "tuple",
            Self::List => "list",
            Self::Curry => "curry",
        })
    }
}

#[derive(Default)]
pub struct ClvmAttr {
    pub repr: Option<Repr>,
    pub untagged: bool,
}

impl ClvmAttr {
    pub fn expect_repr(&self) -> Repr {
        self.repr
            .expect("expected clvm attribute parameter of either `tuple`, `list`, or `curry`")
    }
}

pub fn parse_clvm_attr(attrs: &[Attribute]) -> ClvmAttr {
    let mut result = ClvmAttr::default();
    for attr in attrs {
        let Some(ident) = attr.path().get_ident() else {
            continue;
        };

        if ident != "clvm" {
            continue;
        }

        let args = attr
            .parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)
            .unwrap();

        for arg in args {
            let existing = result.repr;

            result.repr = Some(match arg.to_string().as_str() {
                "tuple" => Repr::Tuple,
                "list" => Repr::List,
                "curry" => Repr::Curry,
                "untagged" => {
                    if result.untagged {
                        panic!("`untagged` specified twice");
                    } else {
                        result.untagged = true;
                    }
                    continue;
                }
                ident => panic!("unknown argument `{ident}`"),
            });

            if let Some(existing) = existing {
                panic!("`{arg}` conflicts with `{existing}`");
            }
        }
    }
    result
}

pub fn parse_int_repr(attrs: &[Attribute]) -> Ident {
    let mut int_repr: Option<Ident> = None;
    for attr in attrs {
        let Some(ident) = attr.path().get_ident() else {
            continue;
        };
        if ident == "repr" {
            int_repr = Some(attr.parse_args_with(Ident::parse_any).unwrap());
        }
    }
    int_repr.unwrap_or(Ident::new("isize", Span::call_site()))
}

pub fn add_trait_bounds(generics: &mut Generics, bound: TypeParamBound) {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bound.clone());
        }
    }
}
