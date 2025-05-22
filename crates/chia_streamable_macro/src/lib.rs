#![allow(clippy::missing_panics_doc)]

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::token::Pub;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, Fields, FieldsNamed, FieldsUnnamed, Index, Lit,
    Type, Visibility,
};

#[proc_macro_attribute]
pub fn streamable(attr: TokenStream, item: TokenStream) -> TokenStream {
    let found_crate =
        crate_name("chia-protocol").expect("chia-protocol is present in `Cargo.toml`");

    let chia_protocol = match &found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(name, Span::call_site());
            quote!(#ident)
        }
    };

    let is_message = &attr.to_string() == "message";
    let is_subclass = &attr.to_string() == "subclass";
    let no_serde = &attr.to_string() == "no_serde";
    let no_json = &attr.to_string() == "no_json";

    let mut input: DeriveInput = parse_macro_input!(item);
    let name = input.ident.clone();
    let name_ref = &name;

    let mut extra_impls = Vec::new();

    if let Data::Struct(data) = &mut input.data {
        let mut field_names = Vec::new();
        let mut field_types = Vec::new();

        for (i, field) in data.fields.iter_mut().enumerate() {
            field.vis = Visibility::Public(Pub::default());
            field_names.push(Ident::new(
                &field
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(format!("field_{i}")),
                Span::mixed_site(),
            ));
            field_types.push(field.ty.clone());
        }

        let init_names = field_names.clone();

        let initializer = match &data.fields {
            Fields::Named(..) => quote!( Self { #( #init_names ),* } ),
            Fields::Unnamed(..) => quote!( Self( #( #init_names ),* ) ),
            Fields::Unit => quote!(Self),
        };

        if field_names.is_empty() {
            extra_impls.push(quote! {
                impl Default for #name_ref {
                    fn default() -> Self {
                        Self::new()
                    }
                }
            });
        }

        extra_impls.push(quote! {
            impl #name_ref {
                #[allow(clippy::too_many_arguments)]
                pub fn new( #( #field_names: #field_types ),* ) -> #name_ref {
                    #initializer
                }
            }
        });

        if is_message {
            extra_impls.push(quote! {
                impl #chia_protocol::ChiaProtocolMessage for #name_ref {
                    fn msg_type() -> #chia_protocol::ProtocolMessageTypes {
                        #chia_protocol::ProtocolMessageTypes::#name_ref
                    }
                }
            });
        }
    } else {
        panic!("only structs are supported");
    }

    let main_derives = quote! {
        #[derive(chia_streamable_macro::Streamable, Hash, Debug, Clone, Eq, PartialEq)]
    };

    let class_attrs = if is_subclass {
        quote!(frozen, subclass)
    } else {
        quote!(frozen)
    };

    // If you're calling the macro from `chia-protocol`, enable Python bindings and arbitrary conditionally.
    // Otherwise, you're calling it from an external crate which doesn't have this infrastructure setup.
    // In that case, the caller can add these macros manually if they want to.
    let attrs = if matches!(found_crate, FoundCrate::Itself) {
        let serde = if is_message || no_serde {
            TokenStream2::default()
        } else {
            quote! {
                #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
            }
        };

        let json_dict = if no_json {
            TokenStream2::default()
        } else {
            quote! {
                #[cfg_attr(feature = "py-bindings", derive(chia_py_streamable_macro::PyJsonDict))]
            }
        };

        quote! {
            #[cfg_attr(
                feature = "py-bindings", pyo3::pyclass(#class_attrs), derive(
                    chia_py_streamable_macro::PyStreamable,
                    chia_py_streamable_macro::PyGetters
                )
            )]
            #json_dict
            #main_derives
            #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
            #serde
        }
    } else {
        main_derives
    };

    quote! {
        #attrs
        #input
        #( #extra_impls )*
    }
    .into()
}

#[proc_macro_derive(Streamable)]
pub fn chia_streamable_macro(input: TokenStream) -> TokenStream {
    let found_crate = crate_name("chia-traits").expect("chia-traits is present in `Cargo.toml`");

    let crate_name = match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(#ident)
        }
    };

    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let mut fnames = Vec::<Ident>::new();
    let mut findices = Vec::<Index>::new();
    let mut ftypes = Vec::<Type>::new();
    match data {
        Data::Enum(e) => {
            let mut names = Vec::<Ident>::new();
            let mut values = Vec::<u8>::new();
            for v in &e.variants {
                names.push(v.ident.clone());
                let Some((_, expr)) = &v.discriminant else {
                    panic!("unsupported enum");
                };
                let Expr::Lit(l) = expr else {
                    panic!("unsupported enum (no literal)");
                };
                let Lit::Int(i) = &l.lit else {
                    panic!("unsupported enum (literal is not integer)");
                };
                values.push(
                    i.base10_parse::<u8>()
                        .expect("unsupported enum (value not u8)"),
                );
            }
            let ret = quote! {
                impl #crate_name::Streamable for #ident {
                    fn update_digest(&self, digest: &mut chia_sha2::Sha256) {
                        <u8 as #crate_name::Streamable>::update_digest(&(*self as u8), digest);
                    }
                    fn stream(&self, out: &mut Vec<u8>) -> #crate_name::chia_error::Result<()> {
                        <u8 as #crate_name::Streamable>::stream(&(*self as u8), out)
                    }
                    fn parse<const TRUSTED: bool>(input: &mut std::io::Cursor<&[u8]>) -> #crate_name::chia_error::Result<Self> {
                        let v = <u8 as #crate_name::Streamable>::parse::<TRUSTED>(input)?;
                        match &v {
                            #(#values => Ok(Self::#names),)*
                            _ => Err(#crate_name::chia_error::Error::InvalidEnum),
                        }
                    }
                }
            };
            return ret.into();
        }
        Data::Union(_) => {
            panic!("Streamable does not support Unions");
        }
        Data::Struct(s) => match s.fields {
            Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
                for (index, f) in unnamed.iter().enumerate() {
                    findices.push(Index::from(index));
                    ftypes.push(f.ty.clone());
                }
            }
            Fields::Unit => {}
            Fields::Named(FieldsNamed { named, .. }) => {
                for f in &named {
                    fnames.push(f.ident.as_ref().unwrap().clone());
                    ftypes.push(f.ty.clone());
                }
            }
        },
    }

    if !fnames.is_empty() {
        let ret = quote! {
            impl #crate_name::Streamable for #ident {
                fn update_digest(&self, digest: &mut chia_sha2::Sha256) {
                    #(self.#fnames.update_digest(digest);)*
                }
                fn stream(&self, out: &mut Vec<u8>) -> #crate_name::chia_error::Result<()> {
                    #(self.#fnames.stream(out)?;)*
                    Ok(())
                }
                fn parse<const TRUSTED: bool>(input: &mut std::io::Cursor<&[u8]>) -> #crate_name::chia_error::Result<Self> {
                    Ok(Self { #( #fnames: <#ftypes as #crate_name::Streamable>::parse::<TRUSTED>(input)?, )* })
                }
            }
        };
        ret.into()
    } else if !findices.is_empty() {
        let ret = quote! {
            impl #crate_name::Streamable for #ident {
                fn update_digest(&self, digest: &mut chia_sha2::Sha256) {
                    #(self.#findices.update_digest(digest);)*
                }
                fn stream(&self, out: &mut Vec<u8>) -> #crate_name::chia_error::Result<()> {
                    #(self.#findices.stream(out)?;)*
                    Ok(())
                }
                fn parse<const TRUSTED: bool>(input: &mut std::io::Cursor<&[u8]>) -> #crate_name::chia_error::Result<Self> {
                    Ok(Self( #( <#ftypes as #crate_name::Streamable>::parse::<TRUSTED>(input)?, )* ))
                }
            }
        };
        ret.into()
    } else {
        // this is an empty type (Unit)
        let ret = quote! {
            impl #crate_name::Streamable for #ident {
                fn update_digest(&self, _digest: &mut chia_sha2::Sha256) {}
                fn stream(&self, _out: &mut Vec<u8>) -> #crate_name::chia_error::Result<()> {
                    Ok(())
                }
                fn parse<const TRUSTED: bool>(_input: &mut std::io::Cursor<&[u8]>) -> #crate_name::chia_error::Result<Self> {
                    Ok(Self{})
                }
            }
        };
        ret.into()
    }
}
