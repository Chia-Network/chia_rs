use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Data, DataStruct, DeriveInput, Fields};

use crate::helpers::{add_trait_bounds, crate_ident, parse_args, Repr};

pub fn from_clvm(mut ast: DeriveInput) -> TokenStream {
    let args = parse_args(&ast.attrs);
    let crate_name = crate_ident();

    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };

    let struct_name = &ast.ident;
    let field_type = fields.iter().map(|field| &field.ty);
    let field_names = fields.iter().map(|field| &field.ident);
    let destructure_names = field_names.clone();

    let (match_macro, destructure_macro) = match args.repr {
        Repr::ProperList => (
            quote!( #crate_name::match_list ),
            quote!( #crate_name::destructure_list ),
        ),
        Repr::Tuple => (
            quote!( #crate_name::match_tuple ),
            quote!( #crate_name::destructure_tuple ),
        ),
        Repr::CurriedArgs => (
            quote!( #crate_name::match_curried_args ),
            quote!( #crate_name::destructure_curried_args ),
        ),
    };

    add_trait_bounds(&mut ast.generics, parse_quote!(#crate_name::FromClvm));
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_name::FromClvm for #struct_name #ty_generics #where_clause {
            fn from_clvm(a: &clvmr::Allocator, node: clvmr::allocator::NodePtr) -> #crate_name::Result<Self> {
                let #destructure_macro!( #( #destructure_names, )* ) = <#match_macro!( #( #field_type ),* ) as #crate_name::FromClvm>::from_clvm(a, node)?;
                Ok(Self { #( #field_names, )* })
            }
        }
    }
}
