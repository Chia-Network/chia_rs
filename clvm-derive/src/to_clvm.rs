use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{parse_quote, Data, DeriveInput, Fields, GenericParam, Index, TypeParam};

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
        Repr::List => quote!( #crate_name::clvm_list ),
        Repr::Tuple => quote!( #crate_name::clvm_tuple ),
        Repr::Curry => quote!( #crate_name::clvm_curried_args ),
    };

    let node_name = Ident::new("Node", Span::mixed_site());

    add_trait_bounds(
        &mut ast.generics,
        parse_quote!(#crate_name::ToClvm<#node_name>),
    );

    let generics_clone = ast.generics.clone();
    let (_, ty_generics, where_clause) = generics_clone.split_for_impl();

    ast.generics
        .params
        .push(GenericParam::Type(TypeParam::from(node_name.clone())));
    let (impl_generics, _, _) = ast.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_name::ToClvm<#node_name> for #struct_name #ty_generics #where_clause {
            fn to_clvm(
                &self,
                encoder: &mut impl #crate_name::ClvmEncoder<Node = #node_name>
            ) -> ::std::result::Result<#node_name, #crate_name::ToClvmError> {
                let value = #list_macro!( #( &self.#field_names ),* );
                #crate_name::ToClvm::to_clvm(&value, encoder)
            }
        }
    }
}
