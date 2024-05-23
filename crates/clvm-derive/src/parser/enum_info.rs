use syn::{DataEnum, Ident};

use super::{parse_clvm_options, parse_variant, ClvmOptions, Repr, VariantInfo};

pub struct EnumInfo {
    pub variants: Vec<VariantInfo>,
    pub discriminant_type: Option<Ident>,
    pub is_untagged: bool,
    pub default_repr: Repr,
    pub crate_name: Option<Ident>,
}

pub fn parse_enum(mut options: ClvmOptions, data_enum: &DataEnum) -> EnumInfo {
    if options.constant.is_some() {
        panic!("`constant` only applies to fields");
    }

    if options.default.is_some() {
        panic!("`default` only applies to fields");
    }

    if options.rest {
        panic!("`rest` only applies to fields");
    }

    let repr = Repr::expect(options.repr);

    if repr == Repr::Transparent {
        if options.untagged {
            panic!("`transparent` enums are implicitly untagged");
        }

        options.untagged = true;
    }

    let mut variants = Vec::new();

    for variant in data_enum.variants.iter() {
        let variant_options = parse_clvm_options(&variant.attrs);
        let variant_repr = variant_options.repr;

        if repr == Repr::Atom && variant_repr.is_some() {
            panic!("cannot override `atom` representation for individual enum variants");
        }

        if repr == Repr::Atom && !variant.fields.is_empty() {
            panic!("cannot have fields in an `atom` enum variant");
        }

        if !options.untagged && variant_repr.is_some() {
            panic!("cannot specify representation for individual enum variants in a tagged enum");
        }

        let mut variant_info = parse_variant(variant_options, variant);

        if (repr == Repr::Transparent && variant_repr.is_none())
            || variant_repr == Some(Repr::Transparent)
        {
            if variant_info.fields.len() != 1 {
                panic!("`transparent` enum variants must have exactly one field");
            }

            variant_info.fields[0].rest = true;
            variant_info.repr = Some(Repr::List);
        }

        variants.push(variant_info);
    }

    EnumInfo {
        variants,
        discriminant_type: options.enum_repr,
        is_untagged: options.untagged,
        default_repr: repr,
        crate_name: options.crate_name,
    }
}
