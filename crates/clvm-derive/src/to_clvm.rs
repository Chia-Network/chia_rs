use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{parse_quote, DeriveInput, GenericParam, Ident, Index};

use crate::{
    crate_name,
    helpers::{add_trait_bounds, variant_discriminants, DiscriminantInfo},
    parser::{parse, EnumInfo, FieldInfo, ParsedInfo, Repr, StructInfo, StructKind, VariantKind},
};

pub fn to_clvm(ast: DeriveInput) -> TokenStream {
    let parsed = parse("ToClvm", &ast);
    let node_name = Ident::new("Node", Span::mixed_site());
    let encoder_name = Ident::new("E", Span::mixed_site());

    match parsed {
        ParsedInfo::Struct(struct_info) => {
            impl_for_struct(ast, struct_info, &node_name, &encoder_name)
        }
        ParsedInfo::Enum(enum_info) => impl_for_enum(ast, &enum_info, &node_name, &encoder_name),
    }
}

fn encode_fields(
    crate_name: &Ident,
    encoder_name: &Ident,
    fields: &[FieldInfo],
    repr: Repr,
) -> TokenStream {
    let mut body = TokenStream::new();
    let mut value_names = Vec::new();

    // Generate the values that need to be encoded for each field.
    // As well as a unique name for each field to reference later.
    for (i, field) in fields.iter().enumerate() {
        let value_name = Ident::new(&format!("field_{i}"), Span::mixed_site());

        if let Some(value) = &field.constant {
            body.extend(quote! {
                // Use the constant's value directly, since it's not in `self`.
                let #value_name = #value;
            });
        }

        value_names.push(value_name);
    }

    let encode_next = match repr {
        Repr::Atom | Repr::Transparent => unreachable!(),
        // Encode `(A . B)` pairs for lists.
        Repr::List | Repr::Solution => quote!(encode_pair),
        // Encode `(c (q . A) B)` pairs for curried arguments.
        Repr::Curry => quote!(encode_curried_arg),
    };

    let initial_value = match repr {
        Repr::Atom | Repr::Transparent => unreachable!(),
        Repr::List | Repr::Solution => {
            quote!(encoder.encode_atom(#crate_name::Atom::Borrowed(&[]))?)
        }
        Repr::Curry => quote!(encoder.encode_atom(#crate_name::Atom::Borrowed(&[1]))?),
    };

    // We're going to build the return value in reverse order, so we need to start with the terminator.
    body.extend(quote! {
        let mut node = #initial_value;
    });

    for (i, field) in fields.iter().enumerate().rev() {
        let value_name = &value_names[i];
        let ty = &field.ty;

        let mut if_body = TokenStream::new();

        // Encode the field value.
        if_body.extend(quote! {
            let value_node = <#ty as #crate_name::ToClvm<#encoder_name>>::to_clvm(&#value_name, encoder)?;
        });

        if field.rest {
            // This field represents the rest of the arguments, so we can replace the terminator with it.
            if_body.extend(quote! {
                node = value_node;
            });
        } else {
            // Prepend the field value to the existing node with a new pair.
            if_body.extend(quote! {
                node = encoder.#encode_next(value_node, node)?;
            });
        }

        if let Some(default) = &field.optional_with_default {
            let default = default.as_ref().map_or_else(
                || quote!(<#ty as ::std::default::Default>::default()),
                ToTokens::to_token_stream,
            );

            // If the field is equal to the default value, don't encode it.
            body.extend(quote! {
                if #value_name != &#default {
                    #if_body
                }
            });
        } else {
            // Encode the field unconditionally if it's not optional.
            body.extend(if_body);
        }
    }

    body
}

fn impl_for_struct(
    ast: DeriveInput,
    struct_info: StructInfo,
    node_name: &Ident,
    encoder_name: &Ident,
) -> TokenStream {
    let crate_name = crate_name(struct_info.crate_name);

    let mut body = TokenStream::new();

    for (i, field) in struct_info.fields.iter().enumerate() {
        // We can't encode fields that are constant, since they aren't on the actual struct.
        if field.constant.is_some() {
            continue;
        }

        // Rename the field so it doesn't clash with anything else in scope such as `node`.
        let value_name = Ident::new(&format!("field_{i}"), Span::mixed_site());

        match struct_info.kind {
            StructKind::Named => {
                let field_name = &field.ident;
                body.extend(quote! {
                    let #value_name = &self.#field_name;
                });
            }
            StructKind::Unnamed => {
                let field_index = Index::from(i);
                body.extend(quote! {
                    let #value_name = &self.#field_index;
                });
            }
            StructKind::Unit => unreachable!(),
        }
    }

    body.extend(encode_fields(
        &crate_name,
        encoder_name,
        &struct_info.fields,
        struct_info.repr,
    ));

    body.extend(quote! {
        Ok(node)
    });

    trait_impl(ast, &crate_name, node_name, encoder_name, &body)
}

fn impl_for_enum(
    ast: DeriveInput,
    enum_info: &EnumInfo,
    node_name: &Ident,
    encoder_name: &Ident,
) -> TokenStream {
    let crate_name = crate_name(enum_info.crate_name.clone());

    let mut variant_destructures = Vec::new();

    for variant in &enum_info.variants {
        let variant_name = &variant.name;

        let field_names: Vec<Ident> = variant
            .fields
            .iter()
            .map(|field| field.ident.clone())
            .collect();

        let value_names: Vec<Ident> = (0..variant.fields.len())
            .map(|i| Ident::new(&format!("field_{i}"), Span::mixed_site()))
            .collect();

        let destructure = match variant.kind {
            VariantKind::Unit => quote!(Self::#variant_name),
            VariantKind::Unnamed => {
                quote!(Self::#variant_name( #( #value_names, )* ))
            }
            VariantKind::Named => {
                quote!(Self::#variant_name { #( #field_names: #value_names, )* })
            }
        };

        variant_destructures.push(destructure);
    }

    let body = if enum_info.is_untagged {
        let mut variant_bodies = Vec::new();

        for variant in &enum_info.variants {
            let repr = variant.repr.unwrap_or(enum_info.default_repr);

            variant_bodies.push(encode_fields(
                &crate_name,
                encoder_name,
                &variant.fields,
                repr,
            ));
        }

        // Encode the variant's fields directly.
        quote! {
            match self {
                #( #variant_destructures => {
                    #variant_bodies
                    Ok(node)
                }, )*
            }
        }
    } else {
        let DiscriminantInfo {
            discriminant_type,
            discriminant_consts,
            discriminant_names,
            variant_names,
        } = variant_discriminants(enum_info);

        if enum_info.default_repr == Repr::Atom {
            // Encode the discriminant by itself as an atom.
            quote! {
                #( #discriminant_consts )*

                match self {
                    #( Self::#variant_names => {
                        <#discriminant_type as #crate_name::ToClvm<#encoder_name>>::to_clvm(
                            &#discriminant_names,
                            encoder,
                        )
                    }, )*
                }
            }
        } else {
            let encode_next = match enum_info.default_repr {
                Repr::Atom | Repr::Transparent => unreachable!(),
                // Encode `(A . B)` pairs for lists.
                Repr::List | Repr::Solution => quote!(encode_pair),
                // Encode `(c (q . A) B)` pairs for curried arguments.
                Repr::Curry => quote!(encode_curried_arg),
            };

            let mut variant_bodies = Vec::new();

            for variant in &enum_info.variants {
                let repr = variant.repr.unwrap_or(enum_info.default_repr);
                variant_bodies.push(encode_fields(
                    &crate_name,
                    encoder_name,
                    &variant.fields,
                    repr,
                ));
            }

            // Encode the discriminant followed by the variant's fields.
            quote! {
                #( #discriminant_consts )*

                match self {
                    #( #variant_destructures => {
                        #variant_bodies

                        let discriminant_node = <#discriminant_type as #crate_name::ToClvm<#encoder_name>>::to_clvm(
                            &#discriminant_names,
                            encoder,
                        )?;

                        encoder.#encode_next( discriminant_node, node )
                    }, )*
                }
            }
        }
    };

    trait_impl(ast, &crate_name, node_name, encoder_name, &body)
}

fn trait_impl(
    mut ast: DeriveInput,
    crate_name: &Ident,
    node_name: &Ident,
    encoder_name: &Ident,
    body: &TokenStream,
) -> TokenStream {
    let type_name = ast.ident;

    // Every generic type must implement `ToClvm` as well in order for the derived type to implement `ToClvm`.
    // This isn't always perfect, but it's how derive macros work.
    add_trait_bounds(
        &mut ast.generics,
        &parse_quote!(#crate_name::ToClvm<#encoder_name>),
    );

    let generics_clone = ast.generics.clone();

    let (_, ty_generics, where_clause) = generics_clone.split_for_impl();

    ast.generics
        .params
        .push(GenericParam::Type(node_name.clone().into()));

    ast.generics.params.push(GenericParam::Type(
        parse_quote!(#encoder_name: #crate_name::ClvmEncoder<Node = #node_name>),
    ));

    let (impl_generics, _, _) = ast.generics.split_for_impl();

    // Generate the final trait implementation.
    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_name::ToClvm<#encoder_name>
        for #type_name #ty_generics #where_clause {
            fn to_clvm(
                &self,
                encoder: &mut #encoder_name
            ) -> ::std::result::Result<#node_name, #crate_name::ToClvmError> {
                #body
            }
        }
    }
}
