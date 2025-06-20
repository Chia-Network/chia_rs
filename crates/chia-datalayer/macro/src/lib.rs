use proc_macro::TokenStream;

#[proc_macro_derive(PythonError)]
pub fn python_error(input: TokenStream) -> TokenStream {
    let input: syn::DeriveInput = syn::parse_macro_input!(input);
    let mut output = TokenStream::new();

    let syn::Data::Enum(input) = input.data else {
        panic!("only enums are supported");
    };

    let names: Vec<proc_macro2::Ident> = input
        .variants
        .iter()
        .map(|variant| quote::format_ident!("{}", variant.ident))
        .collect();
    let python_names: Vec<proc_macro2::Ident> = input
        .variants
        .iter()
        .map(|variant| quote::format_ident!("{}Error", variant.ident))
        .collect();

    output.extend(TokenStream::from(quote::quote!(
        #[cfg(feature = "py-bindings")]
        pub mod python_exceptions {
            use super::*;

            #(
                pyo3::create_exception!(chia_rs.datalayer, #python_names, pyo3::exceptions::PyException);
            )*

            pub fn add_to_module(py: pyo3::marker::Python<'_>, module: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
                use pyo3::prelude::PyModuleMethods;

                #(
                    module.add(stringify!(#python_names), py.get_type::<#python_names>())?;
                )*

                Ok(())
            }
        }

        #[cfg(feature = "py-bindings")]
        impl From<Error> for pyo3::PyErr {
            fn from(err: Error) -> pyo3::PyErr {
                let message = err.to_string();
                match err {
                    #(
                        Error::#names(..) => python_exceptions::#python_names::new_err(message),
                    )*
                }
            }
        }
    )));

    output
}
