use proc_macro2::Ident;
use syn::Attribute;

#[derive(Default)]
pub struct ClvmDeriveArgs {
    pub proper_list: bool,
}

pub fn parse_args(attrs: &[Attribute]) -> ClvmDeriveArgs {
    let mut clvm_derive_args = ClvmDeriveArgs::default();
    for attr in attrs {
        if let Some(ident) = attr.path().get_ident() {
            if ident == "clvm" {
                let attr_args: Ident = attr.parse_args().unwrap();
                if attr_args == "proper_list" {
                    clvm_derive_args.proper_list = true;
                } else {
                    panic!("unknown option {}, expected `proper_list`", ident);
                }
            }
        }
    }
    clvm_derive_args
}
