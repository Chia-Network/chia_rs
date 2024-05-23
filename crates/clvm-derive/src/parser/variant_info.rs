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

    if options.constant.is_some() {
        panic!("`constant` only applies to fields");
    }

    if options.crate_name.is_some() {
        panic!("`crate_name` can't be set on individual enum variants");
    }

    if options.default.is_some() {
        panic!("`default` only applies to fields");
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

    let (kind, fields) = match &variant.fields {
        Fields::Unit => (VariantKind::Unit, Vec::new()),
        Fields::Named(fields) => (VariantKind::Named, parse_named_fields(fields)),
        Fields::Unnamed(fields) => (VariantKind::Unnamed, parse_unnamed_fields(fields)),
    };

    VariantInfo {
        kind,
        name,
        fields,
        discriminant,
        repr,
    }
}
