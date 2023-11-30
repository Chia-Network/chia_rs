extern crate proc_macro;

use proc_macro::{TokenStream};
use syn::{Item, parse_macro_input};

fn attach_wasm_bindgen(input: &TokenStream) -> TokenStream {
    let mut t = input.to_string();
    t = format!("#[wasm_bindgen::prelude::wasm_bindgen(getter_with_clone)] {}", t);
    t.parse().unwrap()
}

#[proc_macro_attribute]
pub fn with_wasm(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut input_maybe_modified = input.clone();

    let parsed_item = parse_macro_input!(input as Item);

    match parsed_item {
        Item::Struct(_) => {
            let shrink_u128 = true;
            if shrink_u128 {
                let mut s = input_maybe_modified.to_string();
                s = s.replace("u128", "u64");
                input_maybe_modified = s.parse().unwrap();
            }
            // println!("{}", &input_maybe_modified);
            attach_wasm_bindgen(&input_maybe_modified)
        },
        _ => {
            attach_wasm_bindgen(&input_maybe_modified)
        }
    }
}
