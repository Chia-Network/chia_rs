extern crate proc_macro;

use syn::{visit_mut::VisitMut, parse_quote, Type};
use proc_macro::{TokenStream};

// See https://github.com/rustwasm/wasm-bindgen/issues/3707
fn _patch_issue_3707(input: &TokenStream) -> TokenStream {
    let mut s = input.to_string();
    s = format!("#[wasm_bindgen::prelude::wasm_bindgen(getter_with_clone)] {}", s);
    s.parse().unwrap()
}

struct TypeConverter;

// As of Dec 2023, wasm-bindgen cannot handle u128 at all.
// We convert u128 into u64 here.
// u64 is then converted into bigint in JS which has no size limitation.
// So this conversion does no harm unless a value greater than u64_max is
// communicated between JS and Wasm Runtime.
impl VisitMut for TypeConverter {
    fn visit_type_mut(&mut self, i: &mut Type) {
        if let Type::Path(type_path) = i {
            if type_path.path.is_ident("u128") {
                *i = parse_quote! { u64 };
            }
        }
        syn::visit_mut::visit_type_mut(self, i);
    }
}

#[proc_macro_attribute]
pub fn with_wasm(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast: syn::File = syn::parse(input.into()).unwrap();
    let mut converter = TypeConverter;
    converter.visit_file_mut(&mut ast);

    let input_maybe_modified = quote::quote!(#ast);
    _patch_issue_3707(&input_maybe_modified.into())
}

#[proc_macro_attribute]
pub fn conv_u128_to_u64_for_wasm(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast: syn::ImplItemFn = syn::parse(input.into()).unwrap();
    let mut converter = TypeConverter;
    converter.visit_impl_item_fn_mut(&mut ast);

    quote::quote!(#ast).into()
}
