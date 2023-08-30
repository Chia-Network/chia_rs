use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{parse_quote, spanned::Spanned, Data, DeriveInput, Fields, Lifetime, Type};

use crate::helpers::{add_trait_bounds, parse_args, Repr};

pub fn parse_tree(mut ast: DeriveInput) -> TokenStream {
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
        Repr::List => (
            quote!( #crate_name::match_list ),
            quote!( #crate_name::destructure_list ),
        ),
        Repr::Tuple => (
            quote!( #crate_name::match_tuple ),
            quote!( #crate_name::destructure_tuple ),
        ),
        Repr::Curry => (
            quote!( #crate_name::match_curried_args ),
            quote!( #crate_name::destructure_curried_args ),
        ),
    };

    let generic_name = Ident::new("__N", Span::call_site());
    let lifetime_name = Lifetime::new("'a", Span::call_site());

    add_trait_bounds(
        &mut ast.generics,
        parse_quote!(#crate_name::ParseTree<#generic_name>),
    );

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let mut tokens = quote!(#impl_generics).into_iter().collect::<Vec<_>>();
    if tokens.len() >= 2 {
        tokens.remove(0);
        tokens.remove(tokens.len() - 1);
    }
    let mut impl_generics = TokenStream::new();
    impl_generics.extend(tokens.into_iter());

    quote! {
        #[automatically_derived]
        impl<#generic_name, #impl_generics> #crate_name::ParseTree<#generic_name>
        for #struct_name #ty_generics #where_clause {
            fn parse_tree<#lifetime_name>(
                f: &impl Fn(#generic_name) -> #crate_name::Value<#lifetime_name, #generic_name>,
                ptr: #generic_name
            ) -> #crate_name::Result<Self> {
                let #destructure_macro!( #( #field_names, )* ) =
                    <#match_macro!( #( #field_types ),* ) as #crate_name::ParseTree<#generic_name>>::parse_tree(f, ptr)?;
                Ok(#initializer)
            }
        }
    }
}
