use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields};

use crate::{args::parse_args, crate_ident::crate_ident};

pub fn impl_from_clvm(ast: DeriveInput) -> TokenStream {
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

    let mut tuple_type = quote! { () };
    for (i, field) in fields.iter().enumerate().rev() {
        let field_type = &field.ty;

        if i == fields.len() - 1 && !args.proper_list {
            tuple_type = quote! { #field_type };
        } else {
            tuple_type = quote! {
                ( #field_type, #tuple_type )
            };
        }
    }

    let mut field_list = Vec::new();

    for (i, field) in fields.iter().enumerate() {
        let field_name = &field.ident;

        let mut tuple_prop = Vec::new();

        for _ in 0..field_list.len() {
            tuple_prop.push(quote! { .1 });
        }

        if i != fields.len() - 1 || args.proper_list {
            tuple_prop.push(quote! { .0 });
        }

        field_list.push(quote! {
            #field_name: values #( #tuple_prop )*
        });
    }

    quote! {
        impl #crate_name::FromClvm for #struct_name {
            fn from_clvm(a: &clvmr::Allocator, node: clvmr::allocator::NodePtr) -> #crate_name::Result<Self> {
                let values = <#tuple_type as #crate_name::FromClvm>::from_clvm(a, node)?;
                Ok(Self { #( #field_list, )* })
            }
        }
    }
}
