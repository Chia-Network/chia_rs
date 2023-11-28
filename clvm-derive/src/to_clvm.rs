use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse_quote, spanned::Spanned, Data, DeriveInput, Fields, FieldsNamed, FieldsUnnamed,
    GenericParam, Index,
};

use crate::{
    helpers::{add_trait_bounds, parse_clvm_attr},
    macros::{repr_macros, Macros},
};

#[derive(Default)]
struct FieldInfo {
    field_names: Vec<Ident>,
    field_accessors: Vec<TokenStream>,
    initializer: TokenStream,
}

pub fn to_clvm(ast: DeriveInput) -> TokenStream {
    let clvm_attr = parse_clvm_attr(&ast.attrs);
    let crate_name = quote!(clvm_traits);

    match &ast.data {
        Data::Struct(data_struct) => {
            if clvm_attr.untagged {
                panic!("cannot use `untagged` on a struct");
            }
            let macros = repr_macros(&crate_name, clvm_attr.expect_repr());
            let field_info = fields(&data_struct.fields);
            impl_for_struct(&crate_name, &ast, &macros, &field_info)
        }
        _ => panic!("expected struct with named or unnamed fields"),
    }
}

fn fields(fields: &Fields) -> FieldInfo {
    match fields {
        Fields::Named(fields) => named_fields(fields),
        Fields::Unnamed(fields) => unnamed_fields(fields),
        Fields::Unit => FieldInfo::default(),
    }
}

fn named_fields(fields: &FieldsNamed) -> FieldInfo {
    let field_names: Vec<Ident> = fields
        .named
        .iter()
        .map(|field| field.ident.clone().unwrap())
        .collect();
    let field_accessors = field_names
        .iter()
        .map(|field_name| field_name.clone().to_token_stream())
        .collect();
    let initializer = quote!({ #( #field_names, )* });

    FieldInfo {
        field_names,
        field_accessors,
        initializer,
    }
}

fn unnamed_fields(fields: &FieldsUnnamed) -> FieldInfo {
    let field_names: Vec<Ident> = fields
        .unnamed
        .iter()
        .enumerate()
        .map(|(i, field)| Ident::new(&format!("field_{i}"), field.span()))
        .collect();
    let field_accessors = field_names
        .iter()
        .enumerate()
        .map(|(i, _)| Index::from(i).to_token_stream())
        .collect();
    let initializer = quote!(( #( #field_names, )* ));

    FieldInfo {
        field_names,
        field_accessors,
        initializer,
    }
}

fn impl_for_struct(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    Macros { clvm_macro, .. }: &Macros,
    FieldInfo {
        field_accessors, ..
    }: &FieldInfo,
) -> TokenStream {
    let node_name = Ident::new("Node", Span::mixed_site());

    let body = quote! {
        let value = #clvm_macro!( #( &self.#field_accessors ),* );
        #crate_name::ToClvm::to_clvm(&value, encoder)
    };

    generate_to_clvm(crate_name, ast, &node_name, &body)
}

fn generate_to_clvm(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    node_name: &Ident,
    body: &TokenStream,
) -> TokenStream {
    let mut ast = ast.clone();
    let item_name = ast.ident;

    add_trait_bounds(
        &mut ast.generics,
        parse_quote!(#crate_name::ToClvm<#node_name>),
    );

    let generics_clone = ast.generics.clone();
    let (_, ty_generics, where_clause) = generics_clone.split_for_impl();

    ast.generics
        .params
        .push(GenericParam::Type(node_name.clone().into()));
    let (impl_generics, _, _) = ast.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_name::ToClvm<#node_name> for #item_name #ty_generics #where_clause {
            fn to_clvm(
                &self,
                encoder: &mut impl #crate_name::ClvmEncoder<Node = #node_name>
            ) -> ::std::result::Result<#node_name, #crate_name::ToClvmError> {
                #body
            }
        }
    }
}
