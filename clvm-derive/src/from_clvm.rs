use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, spanned::Spanned, Data, DeriveInput, Fields, FieldsNamed, FieldsUnnamed,
    GenericParam, Type,
};

use crate::{
    helpers::{add_trait_bounds, parse_clvm_attr},
    macros::{repr_macros, Macros},
};

#[derive(Default)]
struct FieldInfo {
    field_types: Vec<Type>,
    field_names: Vec<Ident>,
    initializer: TokenStream,
}

pub fn from_clvm(ast: DeriveInput) -> TokenStream {
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
    let fields = &fields.named;
    let field_types = fields.iter().map(|field| field.ty.clone()).collect();
    let field_names: Vec<Ident> = fields
        .iter()
        .map(|field| field.ident.clone().unwrap())
        .collect();
    let initializer = quote!({ #( #field_names, )* });

    FieldInfo {
        field_types,
        field_names,
        initializer,
    }
}

fn unnamed_fields(fields: &FieldsUnnamed) -> FieldInfo {
    let fields = &fields.unnamed;
    let field_types = fields.iter().map(|field| field.ty.clone()).collect();
    let field_names: Vec<Ident> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| Ident::new(&format!("field_{i}"), field.span()))
        .collect();
    let initializer = quote!(( #( #field_names, )* ));

    FieldInfo {
        field_types,
        field_names,
        initializer,
    }
}

fn impl_for_struct(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    Macros {
        match_macro,
        destructure_macro,
        ..
    }: &Macros,
    FieldInfo {
        field_types,
        field_names,
        initializer,
    }: &FieldInfo,
) -> TokenStream {
    let node_name = Ident::new("Node", Span::mixed_site());

    let body = quote! {
        let #destructure_macro!( #( #field_names, )* ) =
            <#match_macro!( #( #field_types ),* )
            as #crate_name::FromClvm<#node_name>>::from_clvm(decoder, node)?;
        Ok(Self #initializer)
    };

    generate_from_clvm(crate_name, ast, &node_name, &body)
}

fn generate_from_clvm(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    node_name: &Ident,
    body: &TokenStream,
) -> TokenStream {
    let mut ast = ast.clone();
    let item_name = ast.ident;

    add_trait_bounds(
        &mut ast.generics,
        parse_quote!(#crate_name::FromClvm<#node_name>),
    );

    let generics_clone = ast.generics.clone();
    let (_, ty_generics, where_clause) = generics_clone.split_for_impl();

    ast.generics
        .params
        .push(GenericParam::Type(node_name.clone().into()));
    let (impl_generics, _, _) = ast.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_name::FromClvm<#node_name>
        for #item_name #ty_generics #where_clause {
            fn from_clvm(
                decoder: &impl #crate_name::ClvmDecoder<Node = #node_name>,
                node: #node_name,
            ) -> ::std::result::Result<Self, #crate_name::FromClvmError> {
                #body
            }
        }
    }
}
