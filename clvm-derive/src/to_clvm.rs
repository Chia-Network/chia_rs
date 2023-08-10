use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Data, DataStruct, DeriveInput, Fields};

use crate::helpers::{add_trait_bounds, parse_args, Repr};

pub fn to_clvm(mut ast: DeriveInput) -> TokenStream {
    let args = parse_args(&ast.attrs);
    let crate_name = quote!(clvm_traits);

    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };

    let struct_name = &ast.ident;
    let field_name = fields.iter().map(|field| &field.ident);

    let list_macro = match args.repr {
        Repr::ProperList => quote!( #crate_name::clvm_list ),
        Repr::Tuple => quote!( #crate_name::clvm_tuple ),
        Repr::CurriedArgs => quote!( #crate_name::clvm_curried_args ),
    };

    add_trait_bounds(&mut ast.generics, parse_quote!(#crate_name::ToClvm));
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_name::ToClvm for #struct_name #ty_generics #where_clause {
            fn to_clvm(&self, a: &mut clvmr::Allocator) -> #crate_name::Result<clvmr::allocator::NodePtr> {
                let value = #list_macro!( #( &self.#field_name ),* );
                #crate_name::ToClvm::to_clvm(&value, a)
            }
        }
    }
}
