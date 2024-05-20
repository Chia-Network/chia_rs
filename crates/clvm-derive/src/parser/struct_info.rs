use syn::{DataStruct, Fields, Ident};

use super::{parse_named_fields, parse_unnamed_fields, ClvmOptions, FieldInfo, Repr};

pub struct StructInfo {
    pub kind: StructKind,
    pub fields: Vec<FieldInfo>,
    pub repr: Repr,
    pub crate_name: Option<Ident>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructKind {
    Unit,
    Unnamed,
    Named,
}

pub fn parse_struct(options: ClvmOptions, data_struct: &DataStruct) -> StructInfo {
    if options.untagged {
        panic!("`untagged` only applies to enums");
    }

    if options.enum_repr.is_some() {
        panic!("`repr` only applies to enums");
    }

    if options.hidden_value.is_some() {
        panic!("`hidden_value` only applies to fields");
    }

    if options.default.is_some() {
        panic!("`default` and `optional` only apply to fields");
    }

    if options.rest {
        panic!("`rest` only applies to fields");
    }

    let repr = Repr::expect(options.repr);

    if repr == Repr::Atom {
        panic!("`atom` is not a valid representation for structs");
    }

    let crate_name = options.crate_name;

    match &data_struct.fields {
        Fields::Unit => StructInfo {
            kind: StructKind::Unit,
            fields: Vec::new(),
            repr,
            crate_name,
        },
        Fields::Named(fields) => StructInfo {
            kind: StructKind::Named,
            fields: parse_named_fields(fields),
            repr,
            crate_name,
        },
        Fields::Unnamed(fields) => StructInfo {
            kind: StructKind::Unnamed,
            fields: parse_unnamed_fields(fields),
            repr,
            crate_name,
        },
    }
}
