extern crate proc_macro;

use proc_macro2::{Ident, Span};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{parse_macro_input, DeriveInput, FieldsNamed, FieldsUnnamed};

#[proc_macro_derive(PyStreamable)]
pub fn py_streamable_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let found_crate = crate_name("chia-traits").expect("chia-traits is present in `Cargo.toml`");

    let crate_name = match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(#ident)
        }
    };

    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let fields = match data {
        syn::Data::Struct(s) => s.fields,
        syn::Data::Enum(_) => {
            return quote! {
                impl #crate_name::to_json_dict::ToJsonDict for #ident {
                    fn to_json_dict(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::PyObject> {
                        <u8 as #crate_name::to_json_dict::ToJsonDict>::to_json_dict(&(*self as u8), py)
                    }
                }

                impl #crate_name::from_json_dict::FromJsonDict for #ident {
                    fn from_json_dict(o: &pyo3::PyAny) -> pyo3::PyResult<Self> {
                        let v = <u8 as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(o)?;
                        <Self as #crate_name::Streamable>::parse(&mut std::io::Cursor::<&[u8]>::new(&[v])).map_err(|e| e.into())
                    }
                }

                impl<'a> pyo3::conversion::FromPyObject<'a> for #ident {
                    fn extract(ob: &'a pyo3::PyAny) -> pyo3::PyResult<Self> {
                        let v: u8 = ob.extract()?;
                        <Self as #crate_name::Streamable>::parse(&mut std::io::Cursor::<&[u8]>::new(&[v])).map_err(|e| e.into())
                    }
                }

                impl pyo3::conversion::ToPyObject for #ident {
                    fn to_object(&self, py: pyo3::Python<'_>) -> pyo3::PyObject {
                        pyo3::conversion::ToPyObject::to_object(&(*self as u8), py)
                    }
                }

                impl pyo3::conversion::IntoPy<pyo3::PyObject> for #ident {
                    fn into_py(self, py: pyo3::Python<'_>) -> pyo3::PyObject {
                        pyo3::conversion::ToPyObject::to_object(&(self as u8), py)
                    }
                }
            }
            .into();
        }
        _ => {
            panic!("Streamable only support struct");
        }
    };

    let mut py_protocol = quote! {
        #[pyo3::pymethods]
        impl #ident {
            fn __str__(&self) -> pyo3::PyResult<String> {
                Ok(format!("{:?}", self))
            }

            fn __repr__(&self) -> pyo3::PyResult<String> {
                Ok(format!("{:?}", self))
            }

            fn __richcmp__(&self, other: pyo3::PyRef<Self>, op: pyo3::class::basic::CompareOp) -> pyo3::Py<pyo3::PyAny> {
                use pyo3::class::basic::CompareOp;
                let py = other.py();
                match op {
                    CompareOp::Eq => pyo3::conversion::IntoPy::into_py(self == &*other, py),
                    CompareOp::Ne => pyo3::conversion::IntoPy::into_py(self != &*other, py),
                    _ => py.NotImplemented(),
                }
            }

            fn __hash__(&self) -> pyo3::PyResult<isize> {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(self, &mut hasher);
                Ok(std::hash::Hasher::finish(&hasher) as isize)
            }
        }
    };

    match fields {
        syn::Fields::Named(FieldsNamed { named, .. }) => {
            let mut fnames = Vec::<syn::Ident>::new();
            let mut ftypes = Vec::<syn::Type>::new();
            for f in named.iter() {
                fnames.push(f.ident.as_ref().unwrap().clone());
                ftypes.push(f.ty.clone());
            }

            py_protocol.extend( quote! {
                #[pyo3::pymethods]
                impl #ident {
                    #[allow(too_many_arguments)]
                    #[new]
                    #[pyo3(signature = (#(#fnames),*))]
                    fn new ( #(#fnames : #ftypes),* ) -> Self {
                        Self { #(#fnames),* }
                    }

                    pub fn to_json_dict(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::PyObject> {
                        #crate_name::to_json_dict::ToJsonDict::to_json_dict(self, py)
                    }

                    #[staticmethod]
                    pub fn from_json_dict(o: &pyo3::PyAny) -> pyo3::PyResult<Self> {
                        <Self as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(o)
                    }
                }

                impl #crate_name::to_json_dict::ToJsonDict for #ident {
                    fn to_json_dict(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::PyObject> {
                        let ret = pyo3::types::PyDict::new(py);
                        #(ret.set_item(stringify!(#fnames), self.#fnames.to_json_dict(py)?)?);*;
                        Ok(ret.into())
                    }
                }


                impl #crate_name::from_json_dict::FromJsonDict for #ident {
                    fn from_json_dict(o: &pyo3::PyAny) -> pyo3::PyResult<Self> {
                        Ok(Self{
                            #(#fnames: <#ftypes as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(o.get_item(stringify!(#fnames))?)?,)*
                        })
                    }
                }
            });
        }
        syn::Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
            let mut fnames = Vec::<syn::Ident>::new();
            let mut ftypes = Vec::<syn::Type>::new();
            for (index, f) in unnamed.iter().enumerate() {
                ftypes.push(f.ty.clone());
                fnames.push(syn::Ident::new(&format!("a{index}"), Span::call_site()));
            }

            py_protocol.extend(quote! {
                #[pyo3::pymethods]
                impl #ident {
                    #[new]
                    fn new ( #(#fnames: #ftypes),* ) -> Self {
                        Self ( #(#fnames),* )
                    }
                }
            });

            if fnames.len() == 1 {
                // tuples with a single member support the json protocol by just
                // behaving as their member
                py_protocol.extend(quote! {
                    #[pyo3::pymethods]
                    impl #ident {
                        #[staticmethod]
                        pub fn from_json_dict(o: &pyo3::PyAny) -> pyo3::PyResult<Self> {
                            <Self as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(o)
                        }

                        pub fn to_json_dict(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::PyObject> {
                            #crate_name::to_json_dict::ToJsonDict::to_json_dict(self, py)
                        }
                    }

                    // only types with named fields suport the to/from JSON protocol
                    impl #crate_name::to_json_dict::ToJsonDict for #ident {
                        fn to_json_dict(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::PyObject> {
                            self.0.to_json_dict(py)
                        }
                    }

                    impl #crate_name::from_json_dict::FromJsonDict for #ident {
                        fn from_json_dict(o: &pyo3::PyAny) -> pyo3::PyResult<Self> {
                            Ok(Self(#(<#ftypes as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(o)?)*))
                        }
                    }
                });
            }
        }
        syn::Fields::Unit => {
            panic!("PyStreamable does not support the unit type");
        }
    }

    let streamable = quote! {
        #[pyo3::pymethods]
        impl #ident {
            #[staticmethod]
            pub fn from_bytes(blob: &[u8]) -> pyo3::PyResult<Self> {
                let mut input = std::io::Cursor::<&[u8]>::new(blob);
                <Self as #crate_name::Streamable>::parse(&mut input).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e))
            }

            // returns the type as well as the number of bytes read from the buffer
            #[staticmethod]
            pub fn parse_rust<'p>(blob: pyo3::buffer::PyBuffer<u8>) -> pyo3::PyResult<(Self, u32)> {
                if !blob.is_c_contiguous() {
                    panic!("parse_rust() must be called with a contiguous buffer");
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes())
                };
                let mut input = std::io::Cursor::<&[u8]>::new(slice);
                <Self as #crate_name::Streamable>::parse(&mut input).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e)).map(|v| (v, input.position() as u32))
            }

            pub fn get_hash<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<&'p pyo3::types::PyBytes> {
                let mut ctx = <clvmr::sha2::Sha256 as clvmr::sha2::Digest>::new();
                #crate_name::Streamable::update_digest(self, &mut ctx);
                Ok(pyo3::types::PyBytes::new(py, clvmr::sha2::Digest::finalize(ctx).as_slice()))
            }
            pub fn to_bytes<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<&'p pyo3::types::PyBytes> {
                let mut writer = Vec::<u8>::new();
                #crate_name::Streamable::stream(self, &mut writer).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e))?;
                Ok(pyo3::types::PyBytes::new(py, &writer))
            }

            pub fn __bytes__<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<&'p pyo3::types::PyBytes> {
                self.to_bytes(py)
            }

            pub fn __deepcopy__<'p>(&self, memo: &pyo3::PyAny) -> pyo3::PyResult<Self> {
                Ok(self.clone())
            }

            pub fn __copy__<'p>(&self) -> pyo3::PyResult<Self> {
                Ok(self.clone())
            }
        }
    };
    py_protocol.extend(streamable);
    py_protocol.into()
}
