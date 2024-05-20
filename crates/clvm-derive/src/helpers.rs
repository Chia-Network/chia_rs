use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, Expr, GenericArgument, GenericParam, Generics, Ident, PathArguments, Type,
    TypeParamBound,
};

use crate::parser::EnumInfo;

pub fn add_trait_bounds(generics: &mut Generics, bound: TypeParamBound) {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bound.clone());
        }
    }
}

pub fn option_type(ty: &Type) -> Option<&Type> {
    let Type::Path(ty) = ty else { return None };

    if ty.qself.is_some() {
        return None;
    }

    let ty = &ty.path;

    if ty.segments.is_empty() || ty.segments.last().unwrap().ident != "Option" {
        return None;
    }

    if !(ty.segments.len() == 1
        || (ty.segments.len() == 3
            && ["core", "std"].contains(&ty.segments[0].ident.to_string().as_str())
            && ty.segments[1].ident == "option"))
    {
        return None;
    }

    let last_segment = ty.segments.last().unwrap();

    let PathArguments::AngleBracketed(generics) = &last_segment.arguments else {
        return None;
    };

    if generics.args.len() != 1 {
        return None;
    }

    let GenericArgument::Type(inner_type) = &generics.args[0] else {
        return None;
    };

    Some(inner_type)
}

pub struct DiscriminantInfo {
    pub discriminant_consts: Vec<TokenStream>,
    pub discriminant_names: Vec<Ident>,
    pub variant_names: Vec<Ident>,
    pub discriminant_type: Ident,
}

pub fn variant_discriminants(enum_info: &EnumInfo) -> DiscriminantInfo {
    let mut discriminant_consts = Vec::new();
    let mut discriminant_names = Vec::new();
    let mut variant_names = Vec::new();

    // The default discriminant type is `isize`, but can be overridden with `#[repr(...)]`.
    let discriminant_type = enum_info
        .discriminant_type
        .clone()
        .unwrap_or(Ident::new("isize", Span::mixed_site()));

    // We need to keep track of the previous discriminant to increment it for each variant.
    let mut previous_discriminant = None;

    for (i, variant) in enum_info.variants.iter().enumerate() {
        variant_names.push(variant.name.clone());

        let discriminant = if let Some(expr) = &variant.discriminant {
            // If an explicit discriminant is set, we use that.
            expr.clone()
        } else if let Some(expr) = previous_discriminant {
            // If no explicit discriminant is set, we increment the previous one.
            let expr: Expr = parse_quote!( #expr + 1 );
            expr
        } else {
            // The first variant's discriminant is `0` unless specified otherwise.
            let expr: Expr = parse_quote!(0);
            expr
        };

        previous_discriminant = Some(discriminant.clone());

        // Generate a constant for each variant's discriminant.
        // This is required because you can't directly put an expression inside of a match pattern.
        // So we use a constant to match against instead.
        let discriminant_name = Ident::new(&format!("DISCRIMINANT_{}", i), Span::mixed_site());

        discriminant_names.push(discriminant_name.clone());
        discriminant_consts.push(quote! {
            const #discriminant_name: #discriminant_type = #discriminant;
        });
    }

    DiscriminantInfo {
        discriminant_consts,
        discriminant_names,
        variant_names,
        discriminant_type,
    }
}
