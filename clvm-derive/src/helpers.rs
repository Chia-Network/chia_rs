use std::fmt;

use proc_macro2::Ident;
use syn::{punctuated::Punctuated, Attribute, GenericParam, Generics, Token, TypeParamBound};

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

pub struct ClvmDeriveArgs {
    pub repr: Repr,
}

pub fn parse_args(attrs: &[Attribute]) -> ClvmDeriveArgs {
    let repr = parse_repr(attrs);
    ClvmDeriveArgs {
        repr: repr
            .expect("expected clvm attribute parameter of either `tuple`, `list`, or `curry`"),
    }
}

pub fn parse_repr(attrs: &[Attribute]) -> Option<Repr> {
    let mut repr: Option<Repr> = None;
    for attr in attrs {
        if let Some(ident) = attr.path().get_ident() {
            if ident == "clvm" {
                let args = attr
                    .parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)
                    .unwrap();

                for arg in args {
                    let existing = repr;

                    match arg.to_string().as_str() {
                        "tuple" => repr = Some(Repr::Tuple),
                        "list" => repr = Some(Repr::List),
                        "curry" => repr = Some(Repr::Curry),
                        ident => panic!("unknown argument `{ident}`"),
                    }

                    if let Some(existing) = existing {
                        panic!("`{arg}` conflicts with `{existing}`");
                    }
                }
            }
        }
    }
    repr
}

pub fn add_trait_bounds(generics: &mut Generics, bound: TypeParamBound) {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bound.clone());
        }
    }
}
