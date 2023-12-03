extern crate proc_macro;

mod from_clvm;
mod helpers;
mod macros;
mod to_clvm;

use from_clvm::from_clvm;
use syn::{parse_macro_input, DeriveInput};
use to_clvm::to_clvm;

#[proc_macro_derive(ToClvm, attributes(clvm))]
pub fn to_clvm_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    to_clvm(ast).into()
}

#[proc_macro_derive(FromClvm, attributes(clvm))]
pub fn from_clvm_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    from_clvm(ast).into()
}
