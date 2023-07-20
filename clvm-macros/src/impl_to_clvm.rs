use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields};

use crate::{
    crate_ident::crate_ident,
    parse_args::{parse_args, Repr},
};

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
    let field_name = fields.iter().map(|field| &field.ident);

    let list_macro = match args.repr {
        Repr::ProperList => quote!( #crate_name::clvm_list ),
        Repr::Tuple => quote!( #crate_name::clvm_tuple ),
        Repr::CurriedArgs => quote!( #crate_name::clvm_curried_args ),
    };

    quote! {
        impl #crate_name::ToClvm for #struct_name {
            fn to_clvm(&self, a: &mut clvmr::Allocator) -> #crate_name::Result<clvmr::allocator::NodePtr> {
                let value = #list_macro!( #( self.#field_name ),* );
                #crate_name::ToClvm::to_clvm(&value, a)
            }
        }
    }
}
