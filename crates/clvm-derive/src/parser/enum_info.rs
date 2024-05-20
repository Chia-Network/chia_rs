use syn::{DataEnum, Ident};

use super::{parse_clvm_options, parse_variant, ClvmOptions, Repr, VariantInfo};

pub struct EnumInfo {
    pub variants: Vec<VariantInfo>,
    pub discriminant_type: Option<Ident>,
    pub is_untagged: bool,
    pub default_repr: Repr,
    pub crate_name: Option<Ident>,
}

pub fn parse_enum(options: ClvmOptions, data_enum: &DataEnum) -> EnumInfo {
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

    let mut variants = Vec::new();

    for variant in data_enum.variants.iter() {
        let variant_options = parse_clvm_options(&variant.attrs);

        if repr == Repr::Atom && variant_options.repr.is_some() {
            panic!("cannot override `atom` representation for individual enum variants");
        }

        if repr == Repr::Atom && !variant.fields.is_empty() {
            panic!("cannot have fields in an `atom` enum variant");
        }

        if !options.untagged && variant_options.repr.is_some() {
            panic!("cannot specify representation for individual enum variants in a tagged enum");
        }

        variants.push(parse_variant(variant_options, variant));
    }

    EnumInfo {
        variants,
        discriminant_type: options.enum_repr,
        is_untagged: options.untagged,
        default_repr: repr,
        crate_name: options.crate_name,
    }
}
