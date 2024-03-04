extern crate proc_macro;

use proc_macro2::{Ident, Span};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::Lit::Int;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, Fields, FieldsNamed, FieldsUnnamed, Index, Type,
};

#[proc_macro_derive(Streamable)]
pub fn chia_streamable_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
            for v in e.variants.iter() {
                names.push(v.ident.clone());
                let expr = match &v.discriminant {
                    Some((_, expr)) => expr,
                    None => {
                        panic!("unsupported enum");
                    }
                };
                let l = match expr {
                    Expr::Lit(l) => l,
                    _ => {
                        panic!("unsupported enum (no literal)");
                    }
                };
                let i = match &l.lit {
                    Int(i) => i,
                    _ => {
                        panic!("unsupported enum (literal is not integer)");
                    }
                };
                match i.base10_parse::<u8>() {
                    Ok(v) => values.push(v),
                    Err(_) => {
                        panic!("unsupported enum (value not u8)");
                    }
                }
            }
            let ret = quote! {
                impl #crate_name::Streamable for #ident {
                    fn update_digest(&self, digest: &mut sha2::Sha256) {
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
                for f in named.iter() {
                    fnames.push(f.ident.as_ref().unwrap().clone());
                    ftypes.push(f.ty.clone());
                }
            }
        },
    };

    if !fnames.is_empty() {
        let ret = quote! {
            impl #crate_name::Streamable for #ident {
                fn update_digest(&self, digest: &mut sha2::Sha256) {
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
                fn update_digest(&self, digest: &mut sha2::Sha256) {
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
                fn update_digest(&self, _digest: &mut sha2::Sha256) {}
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
