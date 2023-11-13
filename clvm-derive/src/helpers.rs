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

pub struct Args {
    pub repr: Option<Repr>,
    pub raw_enum: bool,
    pub int_repr: Ident,
}

pub fn parse_args(attrs: &[Attribute]) -> Args {
    let (repr, raw_enum) = parse_repr(attrs);
    let int_repr = parse_int_repr(attrs);

    Args {
        repr,
        raw_enum,
        int_repr: int_repr.unwrap_or(Ident::new("isize", Span::call_site())),
    }
}

pub fn expect_repr(repr: Option<Repr>) -> Repr {
    repr.expect("expected clvm attribute parameter of either `tuple`, `list`, or `curry`")
}

pub fn parse_repr(attrs: &[Attribute]) -> (Option<Repr>, bool) {
    let mut repr: Option<Repr> = None;
    let mut raw_enum = false;
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
                        "raw" => {
                            if raw_enum {
                                panic!("`raw` specified twice");
                            } else {
                                raw_enum = true;
                            }
                            continue;
                        }
                        ident => panic!("unknown argument `{ident}`"),
                    }

                    if let Some(existing) = existing {
                        panic!("`{arg}` conflicts with `{existing}`");
                    }
                }
            }
        }
    }
    (repr, raw_enum)
}

fn parse_int_repr(attrs: &[Attribute]) -> Option<Ident> {
    let mut int_repr: Option<Ident> = None;
    for attr in attrs {
        if let Some(ident) = attr.path().get_ident() {
            if ident == "repr" {
                int_repr = Some(attr.parse_args_with(Ident::parse_any).unwrap());
            }
        }
    }
    int_repr
}

pub fn add_trait_bounds(generics: &mut Generics, bound: TypeParamBound) {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bound.clone());
        }
    }
}
