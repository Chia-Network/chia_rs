use syn::{Data, DeriveInput};

mod attributes;
mod enum_info;
mod field_info;
mod struct_info;
mod variant_info;

pub use attributes::*;
pub use enum_info::*;
pub use field_info::*;
pub use struct_info::*;
pub use variant_info::*;

pub enum ParsedInfo {
    Struct(StructInfo),
    Enum(EnumInfo),
}

pub fn parse(derive: &'static str, ast: &DeriveInput) -> ParsedInfo {
    let options = parse_clvm_options(&ast.attrs);

    match &ast.data {
        Data::Struct(data_struct) => ParsedInfo::Struct(parse_struct(options, data_struct)),
        Data::Enum(data_enum) => ParsedInfo::Enum(parse_enum(options, data_enum)),
        Data::Union(..) => panic!("cannot derive `{derive}` for a union"),
    }
}
