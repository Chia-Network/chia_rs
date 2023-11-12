use proc_macro2::Ident;
use syn::{punctuated::Punctuated, Attribute, GenericParam, Generics, Token, TypeParamBound};

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
                    if let Some(existing) = repr {
                        panic!("`{arg}` conflicts with `{}`", existing.to_string());
                    }

                    repr = Some(match arg.to_string().as_str() {
                        "tuple" => Repr::Tuple,
                        "list" => Repr::List,
                        "curry" => Repr::Curry,
                        ident => panic!("unknown argument `{}`", ident),
                    });
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
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bound.clone());
        }
    }
}
