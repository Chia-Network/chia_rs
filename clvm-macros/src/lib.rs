extern crate proc_macro;

mod crate_ident;
mod impl_from_clvm;
mod impl_to_clvm;
mod parse_args;

use impl_from_clvm::impl_from_clvm;
use impl_to_clvm::impl_to_clvm;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(ToClvm, attributes(clvm))]
pub fn to_clvm_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_to_clvm(ast).into()
}

#[proc_macro_derive(FromClvm, attributes(clvm))]
pub fn from_clvm_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_from_clvm(ast).into()
}
