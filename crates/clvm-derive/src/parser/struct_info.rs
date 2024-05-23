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

    if options.constant.is_some() {
        panic!("`constant` only applies to fields");
    }

    if options.default.is_some() {
        panic!("`default` only applies to fields");
    }

    if options.rest {
        panic!("`rest` only applies to fields");
    }

    let mut repr = Repr::expect(options.repr);

    if repr == Repr::Atom {
        panic!("`atom` is not a valid representation for structs");
    }

    let crate_name = options.crate_name;

    let (kind, mut fields) = match &data_struct.fields {
        Fields::Unit => (StructKind::Unit, Vec::new()),
        Fields::Named(fields) => (StructKind::Named, parse_named_fields(fields)),
        Fields::Unnamed(fields) => (StructKind::Unnamed, parse_unnamed_fields(fields)),
    };

    if repr == Repr::Transparent {
        if fields.len() != 1 {
            panic!("`transparent` structs must have exactly one field");
        }

        fields[0].rest = true;
        repr = Repr::List;
    }

    StructInfo {
        kind,
        fields,
        repr,
        crate_name,
    }
}
