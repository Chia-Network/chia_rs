use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse_quote, spanned::Spanned, Data, DeriveInput, Expr, Fields, FieldsNamed, FieldsUnnamed,
    GenericParam, Index,
};

use crate::{
    helpers::{add_trait_bounds, parse_clvm_attr, Repr},
    macros::{repr_macros, Macros},
};

#[derive(Default)]
struct FieldInfo {
    field_names: Vec<Ident>,
    field_accessors: Vec<TokenStream>,
    initializer: TokenStream,
}

struct VariantInfo {
    name: Ident,
    discriminant: Expr,
    field_info: FieldInfo,
    macros: Macros,
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

            impl_for_enum(&crate_name, &ast, clvm_attr.untagged, &variants)
        }
        Data::Union(_union) => panic!("cannot derive `ToClvm` for a union"),
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

fn impl_for_enum(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    untagged: bool,
    variants: &[VariantInfo],
) -> TokenStream {
    let node_name = Ident::new("Node", Span::mixed_site());
    let has_initializers = variants
        .iter()
        .any(|variant| !variant.field_info.initializer.is_empty());

    let variant_bodies = variants
        .iter()
        .map(|variant_info| {
            let VariantInfo {
                name,
                discriminant,
                field_info,
                macros,
            } = variant_info;

            let FieldInfo {
                field_names,
                initializer,
                ..
            } = field_info;

            let Macros { clvm_macro, .. } = macros;

            if untagged {
                quote! {
                    Self::#name #initializer => {
                        #clvm_macro!( #( #field_names ),* ).to_clvm(encoder)
                    }
                }
            } else if has_initializers {
                quote! {
                    Self::#name #initializer => {
                        (#discriminant, #clvm_macro!( #( #field_names ),* )).to_clvm(encoder)
                    }
                }
            } else {
                quote! {
                    Self::#name => {
                        (#discriminant).to_clvm(encoder)
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let body = quote! {
        match self {
            #( #variant_bodies )*
        }
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
    let type_name = ast.ident;

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
        impl #impl_generics #crate_name::ToClvm<#node_name> for #type_name #ty_generics #where_clause {
            fn to_clvm(
                &self,
                encoder: &mut impl #crate_name::ClvmEncoder<Node = #node_name>
            ) -> ::std::result::Result<#node_name, #crate_name::ToClvmError> {
                #body
            }
        }
    }
}
