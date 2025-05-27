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
    assert!(
        options.constant.is_none(),
        "`constant` only applies to fields"
    );

    assert!(
        options.default.is_none(),
        "`default` only applies to fields"
    );

    assert!(!options.rest, "`rest` only applies to fields");

    let repr = Repr::expect(options.repr);

    if repr == Repr::Transparent {
        assert!(
            !options.untagged,
            "`transparent` enums are implicitly untagged"
        );

        options.untagged = true;
    }

    let mut variants = Vec::new();

    for variant in &data_enum.variants {
        let variant_options = parse_clvm_options(&variant.attrs);
        let variant_repr = variant_options.repr;

        assert!(
            !(repr == Repr::Atom && variant_repr.is_some()),
            "cannot override `atom` representation for individual enum variants"
        );

        assert!(
            repr != Repr::Atom || variant.fields.is_empty(),
            "cannot have fields in an `atom` enum variant"
        );

        assert!(
            options.untagged || variant_repr.is_none(),
            "cannot specify representation for individual enum variants in a tagged enum"
        );

        let mut variant_info = parse_variant(&variant_options, variant);

        if (repr == Repr::Transparent && variant_repr.is_none())
            || variant_repr == Some(Repr::Transparent)
        {
            assert!(
                variant_info.fields.len() == 1,
                "`transparent` enum variants must have exactly one field"
            );

            variant_info.fields[0].rest = true;
            variant_info.repr = Some(Repr::ProperList);
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
