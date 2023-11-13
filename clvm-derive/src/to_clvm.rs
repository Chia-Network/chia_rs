use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse_quote, punctuated::Punctuated, spanned::Spanned, Data, DeriveInput, Expr, Fields,
    FieldsNamed, FieldsUnnamed, GenericParam, Index, TypeParam, TypeParamBound,
};

use crate::{
    helpers::{add_trait_bounds, expect_repr, parse_args, parse_repr, Repr},
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
    value: Expr,
    field_info: FieldInfo,
    macros: Macros,
}

pub fn to_clvm(ast: DeriveInput) -> TokenStream {
    let args = parse_args(&ast.attrs);
    let crate_name = quote!(clvm_traits);

    match &ast.data {
        Data::Struct(data_struct) => {
            if args.raw_enum {
                panic!("cannot use `raw` on a struct");
            }

            let macros = repr_macros(&crate_name, expect_repr(args.repr));
            let field_info = fields(&data_struct.fields);
            impl_for_struct(&crate_name, &ast, &macros, &field_info)
        }
        Data::Enum(data_enum) => {
            if !args.raw_enum && args.repr == Some(Repr::Curry) {
                panic!("cannot use `curry` on an non-raw enum");
            }

            let mut next_value: Expr = parse_quote!(0);
            let mut variants = Vec::new();

            for variant in data_enum.variants.iter() {
                let field_info = fields(&variant.fields);
                let (repr, raw_enum) = parse_repr(&variant.attrs);
                if raw_enum {
                    panic!("cannot use `raw` on an enum variant");
                }
                let repr = repr.or(args.repr);
                if !args.raw_enum && repr == Some(Repr::Curry) {
                    panic!("cannot use `curry` on an non-raw enum");
                }

                let macros = repr_macros(&crate_name, expect_repr(repr));
                let variant_info = VariantInfo {
                    name: variant.ident.clone(),
                    value: variant
                        .discriminant
                        .as_ref()
                        .map(|(_, value)| {
                            if args.raw_enum {
                                panic!("cannot use `raw` on an enum variant with explicit discriminant");
                            }

                            next_value = parse_quote!(#value + 1);
                            value.clone()
                        })
                        .unwrap_or_else(|| {
                            let value = next_value.clone();
                            next_value = parse_quote!(#next_value + 1);
                            value
                        }),
                    field_info,
                    macros,
                };
                variants.push(variant_info);
            }

            impl_for_enum(&crate_name, &ast, args.raw_enum, &variants)
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
        #crate_name::ToClvm::to_clvm(&value, f)
    };

    generate_from_clvm(crate_name, ast, &node_name, &body)
}

fn impl_for_enum(
    crate_name: &TokenStream,
    ast: &DeriveInput,
    raw_enum: bool,
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
                value,
                field_info,
                macros,
            } = variant_info;

            let FieldInfo {
                field_names,
                initializer,
                ..
            } = field_info;

            let Macros { clvm_macro, .. } = macros;

            if raw_enum {
                quote! {
                    Self::#name #initializer => {
                        #clvm_macro!( #( #field_names ),* ).to_clvm(f)
                    }
                }
            } else if has_initializers {
                quote! {
                    Self::#name #initializer => {
                        (#value, #clvm_macro!( #( #field_names ),* )).to_clvm(f)
                    }
                }
            } else {
                quote! {
                    Self::#name => {
                        (#value).to_clvm(f)
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
        parse_quote!(#crate_name::ToClvm<#node_name>),
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
        impl #impl_generics #crate_name::ToClvm<#node_name> for #item_name #ty_generics #where_clause {
            #crate_name::to_clvm!(Node, self, f, {
                #body
            });
        }
    }
}
