use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Data, DeriveInput, Fields, Index};

use crate::helpers::{add_trait_bounds, parse_args, Repr};

pub fn to_clvm(mut ast: DeriveInput) -> TokenStream {
    let args = parse_args(&ast.attrs);
    let crate_name = quote!(clvm_traits);

    let field_names: Vec<TokenStream>;

    match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields) => {
                let fields = &fields.named;
                field_names = fields
                    .iter()
                    .map(|field| {
                        let ident = field.ident.clone().unwrap();
                        quote!(#ident)
                    })
                    .collect();
            }
            Fields::Unnamed(fields) => {
                let fields = &fields.unnamed;
                field_names = fields
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        let index = Index::from(i);
                        quote!(#index)
                    })
                    .collect();
            }
            Fields::Unit => panic!("unit structs are not supported"),
        },
        _ => panic!("expected struct with named or unnamed fields"),
    };

    let struct_name = &ast.ident;

    // `list_macro` encodes a nested tuple containing each of the struct field values within.
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
                let value = #list_macro!( #( &self.#field_names ),* );
                #crate_name::ToClvm::to_clvm(&value, a)
            }
        }
    }
}
