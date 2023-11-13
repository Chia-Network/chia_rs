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
    pub int_repr: Ident,
}

pub fn parse_args(attrs: &[Attribute]) -> ClvmDeriveArgs {
    let mut repr: Option<Repr> = None;
    let mut int_repr: Option<Ident> = None;

    for attr in attrs {
        if let Some(ident) = attr.path().get_ident() {
            match ident.to_string().as_str() {
                "clvm" => {
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
                "repr" => {
                    int_repr = Some(attr.parse_args_with(Ident::parse_any).unwrap());
                }
                _ => {}
            }
        }
    }

    ClvmDeriveArgs {
        repr: repr
            .expect("expected clvm attribute parameter of either `tuple`, `list`, or `curry`"),
        int_repr: int_repr.unwrap_or(Ident::new("isize", Span::call_site())),
    }
}

pub fn add_trait_bounds(generics: &mut Generics, bound: TypeParamBound) {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bound.clone());
        }
    }
}
