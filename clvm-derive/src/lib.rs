extern crate proc_macro;

mod build_tree;
mod from_clvm;
mod helpers;

use build_tree::build_tree;
use from_clvm::from_clvm;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(BuildTree, attributes(clvm))]
pub fn to_clvm_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    build_tree(ast).into()
}

#[proc_macro_derive(FromClvm, attributes(clvm))]
pub fn from_clvm_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    from_clvm(ast).into()
}
