use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, punctuated::Punctuated, spanned::Spanned, Data, DeriveInput, Expr, Fields,
    FieldsNamed, FieldsUnnamed, GenericParam, Type, TypeParam, TypeParamBound,
};

use crate::{
    helpers::{add_trait_bounds, parse_args, Repr},
    macros::{repr_macros, Macros},
};

#[derive(Default)]
struct FieldInfo {
    field_types: Vec<Type>,
    field_names: Vec<Ident>,
    initializer: TokenStream,
}

struct VariantInfo {
    name: Ident,
    value: Expr,
    field_info: FieldInfo,
}

pub fn from_clvm(ast: DeriveInput) -> TokenStream {
    let crate_name = quote!(clvm_traits);
    let args = parse_args(&ast.attrs);
    let macros = repr_macros(&crate_name, args.repr);

    match &ast.data {
        Data::Struct(data_struct) => {
            let field_info = fields(&data_struct.fields);
            impl_for_struct(&crate_name, &ast, &macros, &field_info)
        }
        Data::Enum(data_enum) => {
            if args.repr == Repr::Curry {
                panic!("cannot use `#[clvm(curry)]` on an enum");
            }

            let mut next_value: Expr = parse_quote!(0);
            let mut variants = Vec::new();

            for variant in data_enum.variants.iter() {
                let field_info = fields(&variant.fields);
                let variant_info = VariantInfo {
                    name: variant.ident.clone(),
                    value: variant
                        .discriminant
                        .as_ref()
                        .map(|(_, value)| {
                            next_value = parse_quote!(#value + 1);
                            value.clone()
                        })
                        .unwrap_or_else(|| {
                            let value = next_value.clone();
                            next_value = parse_quote!(#next_value + 1);
                            value
                        }),
                    field_info,
                };
                variants.push(variant_info);
            }

            impl_for_enum(&crate_name, &ast, &args.int_repr, &macros, &variants)
        }
        _ => panic!("expected an enum or struct"),
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
            <#match_macro!( #( #field_types ),* ) as #crate_name::FromClvm<#node_name>>::from_clvm(f, ptr)?;
        Ok(Self #initializer)
    };

    generate_from_clvm(crate_name, ast, &node_name, &body)
}

fn impl_for_enum(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    int_repr: &Ident,
    Macros {
        match_macro,
        destructure_macro,
        ..
    }: &Macros,
    variants: &[VariantInfo],
) -> TokenStream {
    let node_name = Ident::new("Node", Span::mixed_site());

    let mut value_definitions = Vec::new();
    let mut has_initializers = false;

    let variant_bodies = variants
        .iter()
        .enumerate()
        .map(|(i, variant_info)| {
            let VariantInfo {
                name,
                value,
                field_info,
            } = variant_info;

            let FieldInfo {
                field_types,
                field_names,
                initializer,
            } = field_info;

            let value_ident = Ident::new(&format!("VALUE_{}", i), Span::mixed_site());
            value_definitions.push(quote! {
                const #value_ident: #int_repr = #value;
            });

            if initializer.is_empty() {
                quote! {
                    #value_ident => {
                        Ok(Self::#name)
                    }
                }
            } else {
                has_initializers = true;
                quote! {
                    #value_ident => {
                        let #destructure_macro!( #( #field_names ),* ) =
                            <#match_macro!( #( #field_types ),* )
                            as #crate_name::FromClvm<#node_name>>::from_clvm(f, args.0)?;
                        Ok(Self::#name #initializer)
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let parse_value = if has_initializers {
        quote! {
            let (value, args) = <(#int_repr, #crate_name::Raw<#node_name>)>::from_clvm(f, ptr)?;
        }
    } else {
        quote! {
            let value = #int_repr::from_clvm(f, ptr)?;
        }
    };

    let body = quote! {
        #parse_value

        #( #value_definitions )*

        match value {
            #( #variant_bodies )*
            _ => Err(#crate_name::FromClvmError::Invalid(
                format!("unexpected enum variant {value}")
            ))
        }
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

    ast.generics.params.push(GenericParam::Type(TypeParam {
        ident: node_name.clone(),
        attrs: Vec::new(),
        colon_token: None,
        bounds: Punctuated::from_iter([TypeParamBound::Trait(parse_quote!(::std::clone::Clone))]),
        eq_token: None,
        default: None,
    }));
    let (impl_generics, _, _) = ast.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_name::FromClvm<#node_name> for #item_name #ty_generics #where_clause {
            #crate_name::from_clvm!(Node, f, ptr, {
                #body
            });
        }
    }
}
