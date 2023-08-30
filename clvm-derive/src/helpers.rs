use proc_macro2::Ident;
use syn::{punctuated::Punctuated, Attribute, Generics, Token, TypeParamBound};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Repr {
    Tuple,
    List,
    Curry,
}

impl ToString for Repr {
    fn to_string(&self) -> String {
        match self {
            Self::Tuple => "tuple".to_string(),
            Self::List => "list".to_string(),
            Self::Curry => "curry".to_string(),
        }
    }
}

pub struct ClvmDeriveArgs {
    pub repr: Repr,
}

pub fn parse_args(attrs: &[Attribute]) -> ClvmDeriveArgs {
    let mut repr: Option<Repr> = None;

    for attr in attrs {
        if let Some(ident) = attr.path().get_ident() {
            if ident == "clvm" {
                let args = attr
                    .parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)
                    .unwrap();

                for arg in args {
                    match arg.to_string().as_str() {
                        "tuple" => {
                            if let Some(existing) = repr {
                                panic!("`tuple` conflicts with `{}`", existing.to_string());
                            }
                            repr = Some(Repr::Tuple);
                        }
                        "list" => {
                            if let Some(existing) = repr {
                                panic!("`list` conflicts with `{}`", existing.to_string());
                            }
                            repr = Some(Repr::List);
                        }
                        "curry" => {
                            if let Some(existing) = repr {
                                panic!("`curry` conflicts with `{}`", existing.to_string());
                            }
                            repr = Some(Repr::Curry);
                        }
                        ident => panic!("unknown argument `{}`", ident),
                    }
                }
            }
        }
    }

    ClvmDeriveArgs {
        repr: repr
            .expect("expected clvm attribute parameter of either `tuple`, `list`, or `curry`"),
    }
}

pub fn add_trait_bounds(generics: &mut Generics, bound: TypeParamBound) {
    for param in generics.type_params_mut() {
        param.bounds.push(bound.clone());
    }
}
