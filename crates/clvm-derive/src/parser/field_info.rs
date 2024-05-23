use syn::{spanned::Spanned, Expr, FieldsNamed, FieldsUnnamed, Ident, Type};

use super::{parse_clvm_options, ClvmOptions};

pub struct FieldInfo {
    pub ident: Ident,
    pub ty: Type,
    pub constant: Option<Expr>,
    pub optional_with_default: Option<Option<Expr>>,
    pub rest: bool,
}

pub fn parse_named_fields(fields: &FieldsNamed) -> Vec<FieldInfo> {
    let mut items = Vec::new();

    let mut rest = false;
    let mut optional = false;

    for field in fields.named.iter() {
        let ident = field.ident.clone().unwrap();
        let ty = field.ty.clone();

        let options = parse_clvm_options(&field.attrs);
        check_field_options(&options);

        if rest {
            panic!("nothing can come after the `rest` field, since it consumes all arguments");
        }

        if optional {
            panic!("only the last field can be optional, to prevent ambiguity");
        }

        rest = options.rest;
        optional = options.default.is_some();

        items.push(FieldInfo {
            ident,
            ty,
            constant: options.constant,
            optional_with_default: options.default,
            rest: options.rest,
        });
    }

    items
}

pub fn parse_unnamed_fields(fields: &FieldsUnnamed) -> Vec<FieldInfo> {
    let mut items = Vec::new();

    let mut rest = false;
    let mut optional = false;

    for (i, field) in fields.unnamed.iter().enumerate() {
        let ident = Ident::new(&format!("field_{i}"), field.span());
        let ty = field.ty.clone();

        let options = parse_clvm_options(&field.attrs);
        check_field_options(&options);

        if rest {
            panic!("nothing can come after the `rest` field, since it consumes all arguments");
        }

        if optional {
            panic!("only the last field can be optional, to prevent ambiguity");
        }

        rest = options.rest;
        optional = options.default.is_some();

        items.push(FieldInfo {
            ident,
            ty,
            constant: options.constant,
            optional_with_default: options.default,
            rest: options.rest,
        });
    }

    items
}

fn check_field_options(options: &ClvmOptions) {
    if options.untagged {
        panic!("`untagged` only applies to enums");
    }

    if options.enum_repr.is_some() {
        panic!("`repr` only applies to enums");
    }

    if let Some(repr) = options.repr {
        panic!("`{repr}` can't be set on individual fields");
    }

    if options.crate_name.is_some() {
        panic!("`crate_name` can't be set on individual fields");
    }

    if options.default.is_some() && options.constant.is_some() {
        panic!("`default` can't be used with `constant` set");
    }

    if options.default.is_some() && options.rest {
        panic!("`default` can't be used with `rest` option set");
    }
}
