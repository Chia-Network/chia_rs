use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields};

use crate::{
    crate_ident::crate_ident,
    parse_args::{parse_args, Repr},
};

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

    let mut field_list = Vec::new();

    for (i, field) in fields.iter().enumerate() {
        let field_name = &field.ident;

        let mut tuple_prop = Vec::new();

        for _ in 0..field_list.len() {
            match args.repr {
                Repr::Tuple | Repr::ProperList => tuple_prop.push(quote! { .1 }),
                Repr::CurriedArgs => tuple_prop.push(quote! { .1 .1 .0 }),
            }
        }

        let is_last_arg = i == fields.len() - 1;

        match (is_last_arg, args.repr) {
            (true, Repr::Tuple) => (),
            (false, Repr::Tuple) | (_, Repr::ProperList) => tuple_prop.push(quote! { .0 }),
            (_, Repr::CurriedArgs) => tuple_prop.push(quote! { .1 .0 .1 }),
        }

        field_list.push(quote! {
            #field_name: values #( #tuple_prop )*
        });
    }

    let struct_name = &ast.ident;
    let field_type = fields.iter().map(|field| &field.ty);

    let match_macro = match args.repr {
        Repr::ProperList => quote!( #crate_name::match_list ),
        Repr::Tuple => quote!( #crate_name::match_tuple ),
        Repr::CurriedArgs => quote!( #crate_name::match_curried_args ),
    };

    quote! {
        impl #crate_name::FromClvm for #struct_name {
            fn from_clvm(a: &clvmr::Allocator, node: clvmr::allocator::NodePtr) -> #crate_name::Result<Self> {
                let values = <#match_macro!( #( #field_type ),* ) as #crate_name::FromClvm>::from_clvm(a, node)?;
                Ok(Self { #( #field_list, )* })
            }
        }
    }
}
