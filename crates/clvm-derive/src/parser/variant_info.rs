use syn::{Expr, Fields, Ident, Variant};

use super::{parse_named_fields, parse_unnamed_fields, ClvmOptions, FieldInfo, Repr};

pub struct VariantInfo {
    pub kind: VariantKind,
    pub name: Ident,
    pub fields: Vec<FieldInfo>,
    pub discriminant: Option<Expr>,
    pub repr: Option<Repr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantKind {
    Unit,
    Unnamed,
    Named,
}

pub fn parse_variant(options: ClvmOptions, variant: &Variant) -> VariantInfo {
    if options.untagged {
        panic!("`untagged` only applies to enums");
    }

    if options.enum_repr.is_some() {
        panic!("`repr` only applies to enums");
    }

    if options.hidden_value.is_some() {
        panic!("`hidden_value` only applies to fields");
    }

    if options.crate_name.is_some() {
        panic!("`crate_name` can't be set on individual enum variants");
    }

    if options.default.is_some() {
        panic!("`default` and `optional` only apply to fields");
    }

    if options.rest {
        panic!("`rest` only applies to fields");
    }

    let name = variant.ident.clone();
    let discriminant = variant.discriminant.clone().map(|(_, expr)| expr);
    let repr = options.repr;

    if repr == Some(Repr::Atom) {
        panic!("`atom` is not a valid representation for individual enum variants");
    }

    match &variant.fields {
        Fields::Unit => VariantInfo {
            kind: VariantKind::Unit,
            name,
            fields: Vec::new(),
            discriminant,
            repr,
        },
        Fields::Named(fields) => VariantInfo {
            kind: VariantKind::Named,
            name,
            fields: parse_named_fields(fields),
            discriminant,
            repr,
        },
        Fields::Unnamed(fields) => VariantInfo {
            kind: VariantKind::Unnamed,
            name,
            fields: parse_unnamed_fields(fields),
            discriminant,
            repr,
        },
    }
}
