use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, spanned::Spanned, Data, DeriveInput, Expr, Fields, FieldsNamed, FieldsUnnamed,
    GenericParam, Type,
};

use crate::{
    helpers::{add_trait_bounds, parse_clvm_attr, parse_int_repr, Repr},
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
    discriminant: Expr,
    field_info: FieldInfo,
    macros: Macros,
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
        Data::Enum(data_enum) => {
            if !clvm_attr.untagged && clvm_attr.repr == Some(Repr::Curry) {
                panic!("cannot use `curry` on a tagged enum, since unlike other representations, each argument is wrapped");
            }

            let mut next_discriminant: Expr = parse_quote!(0);
            let mut variants = Vec::new();

            for variant in data_enum.variants.iter() {
                let field_info = fields(&variant.fields);
                let variant_clvm_attr = parse_clvm_attr(&variant.attrs);

                if variant_clvm_attr.untagged {
                    panic!("cannot use `untagged` on an enum variant");
                }

                let repr = variant_clvm_attr
                    .repr
                    .unwrap_or_else(|| clvm_attr.expect_repr());
                if !clvm_attr.untagged && repr == Repr::Curry {
                    panic!("cannot use `curry` on a tagged enum variant, since unlike other representations, each argument is wrapped");
                }

                let macros = repr_macros(&crate_name, repr);
                let variant_info = VariantInfo {
                    name: variant.ident.clone(),
                    discriminant: variant
                        .discriminant
                        .as_ref()
                        .map(|(_, discriminant)| {
                            next_discriminant = parse_quote!(#discriminant + 1);
                            discriminant.clone()
                        })
                        .unwrap_or_else(|| {
                            let discriminant = next_discriminant.clone();
                            next_discriminant = parse_quote!(#next_discriminant + 1);
                            discriminant
                        }),
                    field_info,
                    macros,
                };
                variants.push(variant_info);
            }

            if clvm_attr.untagged {
                impl_for_untagged_enum(&crate_name, &ast, &variants)
            } else {
                let int_repr = parse_int_repr(&ast.attrs);
                impl_for_enum(&crate_name, &ast, &int_repr, &variants)
            }
        }
        Data::Union(_union) => panic!("cannot derive `FromClvm` for a union"),
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

fn impl_for_enum(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    int_repr: &Ident,
    variants: &[VariantInfo],
) -> TokenStream {
    let type_name = Literal::string(&ast.ident.to_string());
    let node_name = Ident::new("Node", Span::mixed_site());

    let mut discriminant_definitions = Vec::new();
    let mut has_initializers = false;

    let variant_bodies = variants
        .iter()
        .enumerate()
        .map(|(i, variant_info)| {
            let VariantInfo {
                name,
                discriminant,
                field_info,
                macros,
            } = variant_info;

            let FieldInfo {
                field_types,
                field_names,
                initializer,
            } = field_info;

            let Macros {
                match_macro,
                destructure_macro,
                ..
            } = macros;

            let discriminant_ident = Ident::new(&format!("VALUE_{}", i), Span::mixed_site());
            discriminant_definitions.push(quote! {
                const #discriminant_ident: #int_repr = #discriminant;
            });

            if initializer.is_empty() {
                quote! {
                    #discriminant_ident => {
                        Ok(Self::#name)
                    }
                }
            } else {
                has_initializers = true;
                quote! {
                    #discriminant_ident => {
                        let #destructure_macro!( #( #field_names ),* ) =
                            <#match_macro!( #( #field_types ),* )
                            as #crate_name::FromClvm<#node_name>>::from_clvm(decoder, args.0)?;
                        Ok(Self::#name #initializer)
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let parse_value = if has_initializers {
        quote! {
            let (value, args) = <(#int_repr, #crate_name::Raw<#node_name>)>::from_clvm(decoder, node)?;
        }
    } else {
        quote! {
            let value = #int_repr::from_clvm(decoder, node)?;
        }
    };

    let body = quote! {
        #parse_value

        #( #discriminant_definitions )*

        match value {
            #( #variant_bodies )*
            _ => Err(#crate_name::FromClvmError::Custom(
                format!("failed to match any enum variants of `{}`", #type_name)
            ))
        }
    };

    generate_from_clvm(crate_name, ast, &node_name, &body)
}

fn impl_for_untagged_enum(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    variants: &[VariantInfo],
) -> TokenStream {
    let type_name = Literal::string(&ast.ident.to_string());
    let node_name = Ident::new("Node", Span::mixed_site());

    let variant_bodies = variants
        .iter()
        .map(|variant_info| {
            let VariantInfo {
                name,
                field_info,
                macros,
                ..
            } = variant_info;

            let FieldInfo {
                field_types,
                field_names,
                initializer,
            } = field_info;

            let Macros {
                match_macro,
                destructure_macro,
                ..
            } = macros;

            quote! {
                if let Ok(#destructure_macro!( #( #field_names ),* )) =
                    <#match_macro!( #( #field_types ),* )
                    as #crate_name::FromClvm<#node_name>>::from_clvm(decoder, decoder.clone_node(&node))
                {
                    return Ok(Self::#name #initializer);
                }
            }
        })
        .collect::<Vec<_>>();

    let body = quote! {
        #( #variant_bodies )*

        Err(#crate_name::FromClvmError::Custom(
            format!("failed to match any enum variants of `{}`", #type_name)
        ))
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
    let type_name = ast.ident;

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
        for #type_name #ty_generics #where_clause {
            fn from_clvm(
                decoder: &impl #crate_name::ClvmDecoder<Node = #node_name>,
                node: #node_name,
            ) -> ::std::result::Result<Self, #crate_name::FromClvmError> {
                #body
            }
        }
    }
}
