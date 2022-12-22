extern crate proc_macro;
#[macro_use]
extern crate quote;

use syn::{parse_macro_input, DeriveInput, FieldsNamed, FieldsUnnamed};

use proc_macro::TokenStream;
use quote::__private::Span;

#[proc_macro_derive(PyStreamable)]
pub fn py_streamable_macro(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let fields = match data {
        syn::Data::Struct(s) => s.fields,
        syn::Data::Enum(_) => {
            return quote! {
                impl ToJsonDict for #ident {
                    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
                        <u8 as ToJsonDict>::to_json_dict(&(*self as u8), py)
                    }
                }

                impl FromJsonDict for #ident {
                    fn from_json_dict(o: &pyo3::PyAny) -> PyResult<Self> {
                        let v = <u8 as FromJsonDict>::from_json_dict(o)?;
                        #ident::parse(&mut Cursor::<&[u8]>::new(&[v])).map_err(|e| e.into())
                    }
                }

                impl<'a> pyo3::conversion::FromPyObject<'a> for #ident {
                    fn extract(ob: &'a PyAny) -> PyResult<Self> {
                        let v: u8 = ob.extract()?;
                        Self::parse(&mut Cursor::<&[u8]>::new(&[v])).map_err(|e| e.into())
                    }
                }

                impl pyo3::conversion::ToPyObject for #ident {
                    fn to_object(&self, py: Python<'_>) -> PyObject {
                        (*self as u8).to_object(py)
                    }
                }

                impl pyo3::conversion::IntoPy<PyObject> for #ident {
                    fn into_py(self, py: Python<'_>) -> PyObject {
                        (self as u8).to_object(py)
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
        #[pyproto]
        impl pyo3::class::basic::PyObjectProtocol for #ident {
            fn __str__(&self) -> PyResult<String> {
                Ok(format!("{:?}", self))
            }

            fn __repr__(&self) -> PyResult<String> {
                Ok(format!("{:?}", self))
            }

            fn __richcmp__(&self, other: PyRef<#ident>, op: pyo3::class::basic::CompareOp) -> Py<pyo3::PyAny> {
                use pyo3::class::basic::CompareOp;
                let py = other.py();
                match op {
                    CompareOp::Eq => (self == &*other).into_py(py),
                    CompareOp::Ne => (self != &*other).into_py(py),
                    _ => py.NotImplemented(),
                }
            }

            fn __hash__(&self) -> PyResult<isize> {
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
                #[pymethods]
                impl #ident {
                    #[allow(too_many_arguments)]
                    #[new]
                    fn new ( #(#fnames : #ftypes),* ) -> #ident {
                        #ident { #(#fnames),* }
                    }

                    pub fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
                        ToJsonDict::to_json_dict(self, py)
                    }

                    #[staticmethod]
                    pub fn from_json_dict(o: &pyo3::PyAny) -> PyResult<Self> {
                        <Self as FromJsonDict>::from_json_dict(o)
                    }

                    #(#[getter]
                        fn #fnames(&self, py: Python) -> PyResult<#ftypes> {
                            Ok(self.#fnames.clone())
                        }
                    )*
                }

                impl ToJsonDict for #ident {
                    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
                        let ret = pyo3::types::PyDict::new(py);
                        #(ret.set_item(stringify!(#fnames), self.#fnames.to_json_dict(py)?)?);*;
                        Ok(ret.into())
                    }
                }


                impl FromJsonDict for #ident {
                    fn from_json_dict(o: &pyo3::PyAny) -> PyResult<Self> {
                        Ok(Self{
                            #(#fnames: <#ftypes as FromJsonDict>::from_json_dict(o.get_item(stringify!(#fnames))?)?,)*
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
                #[pymethods]
                impl #ident {
                    #[new]
                    fn new ( #(#fnames: #ftypes),* ) -> #ident {
                        #ident ( #(#fnames),* )
                    }
                }
            });

            if fnames.len() == 1 {
                // tuples with a single member support the json protocol by just
                // behaving as their member
                py_protocol.extend(quote! {
                    #[pymethods]
                    impl #ident {
                        #[staticmethod]
                        pub fn from_json_dict(o: &pyo3::PyAny) -> PyResult<Self> {
                            <Self as FromJsonDict>::from_json_dict(o)
                        }

                        pub fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
                            ToJsonDict::to_json_dict(self, py)
                        }
                    }

                    // only types with named fields suport the to/from JSON protocol
                    impl ToJsonDict for #ident {
                        fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
                            self.0.to_json_dict(py)
                        }
                    }

                    impl FromJsonDict for #ident {
                        fn from_json_dict(o: &pyo3::PyAny) -> PyResult<Self> {
                            Ok(Self(#(<#ftypes as FromJsonDict>::from_json_dict(o)?)*))
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
        #[pymethods]
        impl #ident {
            #[staticmethod]
            pub fn from_bytes(blob: &[u8]) -> PyResult<Self> {
                let mut input = std::io::Cursor::<&[u8]>::new(blob);
                Self::parse(&mut input).map_err(|e| <chia_error::Error as Into<PyErr>>::into(e))
            }

            // returns the type as well as the number of bytes read from the buffer
            #[staticmethod]
            pub fn parse_rust<'p>(blob: pyo3::buffer::PyBuffer<u8>) -> PyResult<(Self, u32)> {
                if !blob.is_c_contiguous() {
                    panic!("parse_rust() must be called with a contiguous buffer");
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes())
                };
                let mut input = std::io::Cursor::<&[u8]>::new(slice);
                Self::parse(&mut input).map_err(|e| <chia_error::Error as Into<PyErr>>::into(e)).map(|v| (v, input.position() as u32))
            }

            pub fn get_hash<'p>(&self, py: Python<'p>) -> PyResult<&'p pyo3::types::PyBytes> {
                let mut ctx = <clvmr::sha2::Sha256 as clvmr::sha2::Digest>::new();
                Streamable::update_digest(self, &mut ctx);
                Ok(pyo3::types::PyBytes::new(py, clvmr::sha2::Digest::finalize(ctx).as_slice()))
            }
            pub fn to_bytes<'p>(&self, py: Python<'p>) -> PyResult<&'p pyo3::types::PyBytes> {
                let mut writer = Vec::<u8>::new();
                self.stream(&mut writer).map_err(|e| <chia_error::Error as Into<PyErr>>::into(e))?;
                Ok(pyo3::types::PyBytes::new(py, &writer))
            }

            pub fn __bytes__<'p>(&self, py: Python<'p>) -> PyResult<&'p pyo3::types::PyBytes> {
                self.to_bytes(py)
            }

            pub fn __deepcopy__<'p>(&self, memo: &pyo3::PyAny) -> PyResult<Self> {
                Ok(self.clone())
            }

            pub fn __copy__<'p>(&self) -> PyResult<Self> {
                Ok(self.clone())
            }
        }
    };
    py_protocol.extend(streamable);
    py_protocol.into()
}
