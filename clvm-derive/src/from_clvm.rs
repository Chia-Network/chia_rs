use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_quote, spanned::Spanned, Data, DeriveInput, Fields, Type};

use crate::helpers::{add_trait_bounds, parse_args, Repr};

pub fn from_clvm(mut ast: DeriveInput) -> TokenStream {
    let args = parse_args(&ast.attrs);
    let crate_name = quote!(clvm_traits);

    let field_types: Vec<Type>;
    let field_names: Vec<Ident>;
    let initializer: TokenStream;

    match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields) => {
                let fields = &fields.named;
                field_types = fields.iter().map(|field| field.ty.clone()).collect();
                field_names = fields
                    .iter()
                    .map(|field| field.ident.clone().unwrap())
                    .collect();
                initializer = quote!(Self { #( #field_names, )* });
            }
            Fields::Unnamed(fields) => {
                let fields = &fields.unnamed;
                field_types = fields.iter().map(|field| field.ty.clone()).collect();
                field_names = fields
                    .iter()
                    .enumerate()
                    .map(|(i, field)| Ident::new(&format!("field_{i}"), field.span()))
                    .collect();
                initializer = quote!(Self( #( #field_names, )* ));
            }
            Fields::Unit => panic!("unit structs are not supported"),
        },
        _ => panic!("expected struct with named or unnamed fields"),
    };

    let struct_name = &ast.ident;

    // `match_macro` decodes a nested tuple containing each of the struct field types within.
    // `destructure_macro` destructures the values into the field names, to be stored in the struct.
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
                let #destructure_macro!( #( #field_names, )* ) = <#match_macro!( #( #field_types ),* ) as #crate_name::FromClvm>::from_clvm(a, node)?;
                Ok(#initializer)
            }
        }
    }
}
