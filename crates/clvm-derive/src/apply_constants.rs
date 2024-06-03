use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Data, DeriveInput, Fields};

use crate::parser::parse_clvm_options;

pub fn impl_apply_constants(mut ast: DeriveInput) -> TokenStream {
    match &mut ast.data {
        Data::Enum(data_enum) => {
            for variant in &mut data_enum.variants {
                remove_fields(&mut variant.fields);
            }
        }
        Data::Struct(data_struct) => {
            remove_fields(&mut data_struct.fields);
        }
        Data::Union(_data_union) => {}
    }

    ast.into_token_stream()
}

fn remove_fields(fields: &mut Fields) {
    match fields {
        Fields::Named(fields) => {
            let retained_fields = fields
                .named
                .clone()
                .into_iter()
                .filter(|field| parse_clvm_options(&field.attrs).constant.is_none());

            fields.named = retained_fields.collect();
        }
        Fields::Unnamed(fields) => {
            let retained_fields = fields
                .unnamed
                .clone()
                .into_iter()
                .filter(|field| parse_clvm_options(&field.attrs).constant.is_none());

            fields.unnamed = retained_fields.collect();
        }
        Fields::Unit => {}
    }
}
