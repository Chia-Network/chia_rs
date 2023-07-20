use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields};

use crate::{args::parse_args, crate_ident::crate_ident};

pub fn impl_to_clvm(ast: DeriveInput) -> TokenStream {
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

    let mut tuple_value = quote! { () };

    for (i, field) in fields.iter().enumerate().rev() {
        let field_name = &field.ident;

        if i == fields.len() - 1 && !args.proper_list {
            tuple_value = quote! { self.#field_name };
        } else {
            tuple_value = quote! {
                ( self.#field_name, #tuple_value )
            };
        }
    }

    quote! {
        impl #crate_name::ToClvm for #struct_name {
            fn to_clvm(&self, a: &mut clvmr::Allocator) -> #crate_name::Result<clvmr::allocator::NodePtr> {
                #crate_name::ToClvm::to_clvm(&#tuple_value, a)
            }
        }
    }
}
