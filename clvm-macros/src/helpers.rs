use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{punctuated::Punctuated, Attribute, GenericParam, Generics, Token, TypeParamBound};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Repr {
    Tuple,
    ProperList,
    CurriedArgs,
}

impl ToString for Repr {
    fn to_string(&self) -> String {
        match self {
            Self::Tuple => "tuple".to_string(),
            Self::ProperList => "proper_list".to_string(),
            Self::CurriedArgs => "curried_list".to_string(),
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
                        "proper_list" => {
                            if let Some(existing) = repr {
                                panic!("`proper_list` conflicts with `{}`", existing.to_string());
                            }
                            repr = Some(Repr::ProperList);
                        }
                        "curried_args" => {
                            if let Some(existing) = repr {
                                panic!("`curried_args` conflicts with `{}`", existing.to_string());
                            }
                            repr = Some(Repr::CurriedArgs);
                        }
                        ident => panic!("unknown argument `{}`", ident),
                    }
                }
            }
        }
    }

    ClvmDeriveArgs {
        repr: repr.unwrap_or(Repr::Tuple),
    }
}

pub fn crate_ident() -> TokenStream {
    let found_crate = crate_name("clvm-utils").expect("`clvm-utils` not found in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(#ident)
        }
    }
}

pub fn add_trait_bounds(generics: &mut Generics, bound: TypeParamBound) {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bound.clone());
        }
    }
}
