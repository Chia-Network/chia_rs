use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{parse_quote, DeriveInput, GenericParam, Ident};

use crate::{
    crate_name,
    helpers::{add_trait_bounds, variant_discriminants, DiscriminantInfo},
    parser::{parse, EnumInfo, FieldInfo, ParsedInfo, Repr, StructInfo, StructKind, VariantKind},
};

pub fn from_clvm(ast: DeriveInput) -> TokenStream {
    let parsed = parse("FromClvm", &ast);
    let node_name = Ident::new("Node", Span::mixed_site());
    let decoder_name = Ident::new("D", Span::mixed_site());

    match parsed {
        ParsedInfo::Struct(struct_info) => {
            impl_for_struct(ast, struct_info, &node_name, &decoder_name)
        }
        ParsedInfo::Enum(enum_info) => impl_for_enum(ast, &enum_info, &node_name, &decoder_name),
    }
}

struct ParsedFields {
    decoded_names: Vec<Ident>,
    decoded_values: Vec<TokenStream>,
    body: TokenStream,
}

fn field_parser_fn_body(
    crate_name: &Ident,
    decoder_name: &Ident,
    fields: &[FieldInfo],
    repr: Repr,
) -> ParsedFields {
    let mut body = TokenStream::new();

    // Generate temporary names for the fields, used in the function body.
    let temp_names: Vec<Ident> = (0..fields.len())
        .map(|i| Ident::new(&format!("field_{i}"), Span::mixed_site()))
        .collect();

    let decode_next = match repr {
        Repr::Atom | Repr::Transparent => unreachable!(),
        // Decode `(A . B)` pairs for lists.
        Repr::List | Repr::Solution => quote!(decode_pair),
        // Decode `(c (q . A) B)` pairs for curried arguments.
        Repr::Curry => quote!(decode_curried_arg),
    };

    let mut optional = false;

    for (i, field) in fields.iter().enumerate() {
        let ident = &temp_names[i];

        if field.rest {
            // Consume the rest of the `node` as the final argument.
            body.extend(quote! {
                let #ident = node;
            });
        } else if field.optional_with_default.is_some() {
            // We need to start tracking the `node` as being optional going forward.
            if !optional {
                body.extend(quote! {
                    let optional_node = Some(decoder.clone_node(&node));
                });
            }

            optional = true;

            // Decode the pair and assign the `Option<Node>` value to the field.
            body.extend(quote! {
                let (#ident, optional_node) = optional_node.and_then(|node| decoder.#decode_next(&node).ok())
                    .map(|(a, b)| (Some(a), Some(b))).unwrap_or((None, None));

                if let Some(new_node) = optional_node.as_ref().map(|node| decoder.clone_node(node)) {
                    node = new_node;
                }
            });
        } else {
            // Otherwise, simply decode a pair and return an error if it fails.
            body.extend(quote! {
                let (#ident, new_node) = decoder.#decode_next(&node)?;
                node = new_node;
            });
        }
    }

    if !fields.last().is_some_and(|field| field.rest) {
        body.extend(check_rest_value(crate_name, repr));
    }

    let mut decoded_names = Vec::new();
    let mut decoded_values = Vec::new();

    for (i, field) in fields.iter().enumerate() {
        let ident = &temp_names[i];
        let ty = &field.ty;

        // This handles the actual decoding of the field's value.
        let mut decoded_value = quote! {
            <#ty as #crate_name::FromClvm<#decoder_name>>::from_clvm(decoder, #ident)
        };

        if let Some(default) = &field.optional_with_default {
            let default = default.as_ref().map_or_else(
                || quote!(<#ty as ::std::default::Default>::default()),
                ToTokens::to_token_stream,
            );

            // If there's a default value, we need to use it instead if the field isn't present.
            decoded_value = quote! {
                #ident.map(|#ident| #decoded_value).unwrap_or(Ok(#default))?
            };
        } else {
            // If the field isn't optional, we can simply return any parsing errors early for this field.
            decoded_value = quote!(#decoded_value?);
        }

        let field_ident = field.ident.clone();

        if let Some(value) = &field.constant {
            // If the field is constant, we need to check that the value is correct before continuing.
            body.extend(quote! {
                let value: #ty = #value;

                if #decoded_value != value {
                    return Err(#crate_name::FromClvmError::Custom(
                        format!(
                            "constant `{}` has an incorrect value",
                            stringify!(#field_ident),
                        )
                    ));
                }
            });
        } else {
            // Otherwise, we can include the field name and decoded value in the constructor.
            decoded_names.push(field_ident);
            decoded_values.push(decoded_value);
        }
    }

    ParsedFields {
        decoded_names,
        decoded_values,
        body,
    }
}

fn check_rest_value(crate_name: &Ident, repr: Repr) -> TokenStream {
    match repr {
        Repr::Atom | Repr::Transparent => unreachable!(),
        // We don't need to check the terminator of a solution.
        Repr::Solution => quote! {},
        Repr::List => {
            // If the last field is not `rest`, we need to check that the `node` is nil.
            // If it's not nil, it's not a proper list, and we should return an error.
            quote! {
                let atom = decoder.decode_atom(&node)?;
                let atom_ref = atom.as_ref();
                if !atom_ref.is_empty() {
                    return Err(#crate_name::FromClvmError::WrongAtomLength {
                        expected: 0,
                        found: atom_ref.len(),
                    });
                }
            }
        }
        Repr::Curry => {
            // Do the same for curried arguments, but check for a terminator of `1` instead.
            // This is because `1` points to the all of the arguments in the program's environment.
            quote! {
                let atom = decoder.decode_atom(&node)?;
                let atom_ref = atom.as_ref();
                if atom_ref.len() != 1 {
                    return Err(#crate_name::FromClvmError::WrongAtomLength {
                        expected: 1,
                        found: atom_ref.len(),
                    });
                }
                if atom_ref != &[1] {
                    return Err(#crate_name::FromClvmError::Custom(
                        "expected curried argument terminator of 1".to_string(),
                    ));
                }
            }
        }
    }
}

fn impl_for_struct(
    ast: DeriveInput,
    struct_info: StructInfo,
    node_name: &Ident,
    decoder_name: &Ident,
) -> TokenStream {
    let crate_name = crate_name(struct_info.crate_name);

    let ParsedFields {
        decoded_names,
        decoded_values,
        mut body,
    } = field_parser_fn_body(
        &crate_name,
        decoder_name,
        &struct_info.fields,
        struct_info.repr,
    );

    // Generate the constructor for the return value, if all parsing was successful.
    match struct_info.kind {
        StructKind::Unit => {
            body.extend(quote!(Ok(Self)));
        }
        StructKind::Unnamed => {
            body.extend(quote! {
                Ok(Self ( #( #decoded_values, )* ))
            });
        }
        StructKind::Named => {
            body.extend(quote! {
                Ok(Self {
                    #( #decoded_names: #decoded_values, )*
                })
            });
        }
    }

    trait_impl(ast, &crate_name, node_name, decoder_name, &body)
}

fn impl_for_enum(
    ast: DeriveInput,
    enum_info: &EnumInfo,
    node_name: &Ident,
    decoder_name: &Ident,
) -> TokenStream {
    let crate_name = crate_name(enum_info.crate_name.clone());

    let mut body = TokenStream::new();

    if enum_info.is_untagged {
        let variant_parsers = enum_variant_parsers(&crate_name, node_name, decoder_name, enum_info);

        // If the enum is untagged, we need to try each variant parser until one succeeds.
        for parser in variant_parsers {
            body.extend(quote! {
                if let Ok(value) = (#parser)(decoder.clone_node(&node)) {
                    return Ok(value);
                }
            });
        }

        body.extend(quote! {
            Err(#crate_name::FromClvmError::Custom(
                "failed to parse any enum variant".to_string(),
            ))
        });
    } else {
        let DiscriminantInfo {
            discriminant_type,
            discriminant_consts,
            discriminant_names,
            variant_names,
        } = variant_discriminants(enum_info);

        if enum_info.default_repr == Repr::Atom {
            // If the enum is represented as an atom, we can simply decode the discriminant and match against it.
            body.extend(quote! {
                let discriminant = <#discriminant_type as #crate_name::FromClvm<#decoder_name>>::from_clvm(
                    decoder,
                    node,
                )?;

                #( #discriminant_consts )*

                match discriminant {
                    #( #discriminant_names => Ok(Self::#variant_names), )*
                    _ => Err(#crate_name::FromClvmError::Custom(
                        format!("unknown enum variant discriminant: {}", discriminant),
                    )),
                }
            });
        } else {
            let variant_parsers =
                enum_variant_parsers(&crate_name, node_name, decoder_name, enum_info);

            let decode_next = match enum_info.default_repr {
                Repr::Atom | Repr::Transparent => unreachable!(),
                // Decode `(A . B)` pairs for lists.
                Repr::List | Repr::Solution => quote!(decode_pair),
                // Decode `(c (q . A) B)` pairs for curried arguments.
                Repr::Curry => quote!(decode_curried_arg),
            };

            // If the enum is represented as a list or curried argument, we need to decode the discriminant first.
            // Then we can match against it to determine which variant to parse.
            body.extend(quote! {
                let (discriminant_node, node) = decoder.#decode_next(&node)?;

                let discriminant = <#discriminant_type as #crate_name::FromClvm<#decoder_name>>::from_clvm(
                    decoder,
                    discriminant_node,
                )?;

                #( #discriminant_consts )*

                match discriminant {
                    #( #discriminant_names => (#variant_parsers)(node), )*
                    _ => Err(#crate_name::FromClvmError::Custom(
                        format!("unknown enum variant discriminant: {}", discriminant),
                    )),
                }
            });
        }
    }

    trait_impl(ast, &crate_name, node_name, decoder_name, &body)
}

fn enum_variant_parsers(
    crate_name: &Ident,
    node_name: &Ident,
    decoder_name: &Ident,
    enum_info: &EnumInfo,
) -> Vec<TokenStream> {
    let mut variant_parsers = Vec::new();

    for variant in &enum_info.variants {
        let variant_name = &variant.name;
        let repr = variant.repr.unwrap_or(enum_info.default_repr);

        let ParsedFields {
            decoded_names,
            decoded_values,
            mut body,
        } = field_parser_fn_body(crate_name, decoder_name, &variant.fields, repr);

        match variant.kind {
            VariantKind::Unit => {
                body.extend(quote!(Ok(Self::#variant_name)));
            }
            VariantKind::Unnamed => {
                body.extend(quote! {
                    Ok(Self::#variant_name ( #( #decoded_values, )* ))
                });
            }
            VariantKind::Named => {
                body.extend(quote! {
                    Ok(Self::#variant_name {
                        #( #decoded_names: #decoded_values, )*
                    })
                });
            }
        };

        // Generate a function that parses the variant's fields and returns the variant or an error.
        // It takes a `node` so that you can pass it a clone of the original `node` to parse from.
        // It uses a reference to the `decoder` from the outer scope as well.
        variant_parsers.push(quote! {
            |mut node: #node_name| -> ::std::result::Result<Self, #crate_name::FromClvmError> {
                #body
            }
        });
    }

    variant_parsers
}

// This generates the `FromClvm` trait implementation and augments generics with the `FromClvm` bound.
fn trait_impl(
    mut ast: DeriveInput,
    crate_name: &Ident,
    node_name: &Ident,
    decoder_name: &Ident,
    body: &TokenStream,
) -> TokenStream {
    let type_name = ast.ident;

    // Every generic type must implement `FromClvm` as well in order for the derived type to implement `FromClvm`.
    // This isn't always perfect, but it's how derive macros work.
    add_trait_bounds(
        &mut ast.generics,
        &parse_quote!(#crate_name::FromClvm<#decoder_name>),
    );

    let generics_clone = ast.generics.clone();

    let (_, ty_generics, where_clause) = generics_clone.split_for_impl();

    ast.generics
        .params
        .push(GenericParam::Type(node_name.clone().into()));

    ast.generics.params.push(GenericParam::Type(
        parse_quote!(#decoder_name: #crate_name::ClvmDecoder<Node = #node_name>),
    ));

    let (impl_generics, _, _) = ast.generics.split_for_impl();

    // Generate the final trait implementation.
    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_name::FromClvm<#decoder_name>
        for #type_name #ty_generics #where_clause {
            fn from_clvm(
                decoder: &#decoder_name,
                mut node: #node_name,
            ) -> ::std::result::Result<Self, #crate_name::FromClvmError> {
                #body
            }
        }
    }
}
