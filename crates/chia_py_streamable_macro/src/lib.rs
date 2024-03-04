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
                impl<'a> pyo3::conversion::FromPyObject<'a> for #ident {
                    fn extract(ob: &'a pyo3::PyAny) -> pyo3::PyResult<Self> {
                        let v: u8 = ob.extract()?;
                        <Self as #crate_name::Streamable>::parse::<false>(&mut std::io::Cursor::<&[u8]>::new(&[v])).map_err(|e| e.into())
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
            fn __repr__(&self) -> pyo3::PyResult<String> {
                Ok(format!("{self:?}"))
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

        impl #crate_name::ChiaToPython for #ident {
            fn to_python<'a>(&self, py: pyo3::Python<'a>) -> pyo3::PyResult<&'a pyo3::PyAny> {
                Ok(pyo3::IntoPy::into_py(self.clone(), py).into_ref(py))
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

            py_protocol.extend(quote! {
                #[pyo3::pymethods]
                impl #ident {
                    #[allow(too_many_arguments)]
                    #[new]
                    #[pyo3(signature = (#(#fnames),*))]
                    pub fn new ( #(#fnames : #ftypes),* ) -> Self {
                        Self { #(#fnames),* }
                    }
                }
            });

            py_protocol.extend(quote! {
                #[pyo3::pymethods]
                impl #ident {
                    #[pyo3(signature = (**kwargs))]
                    fn replace(&self, kwargs: Option<&pyo3::types::PyDict>) -> pyo3::PyResult<Self> {
                        let mut ret = self.clone();
                        if let Some(kwargs) = kwargs {
                            let iter: pyo3::types::iter::PyDictIterator = kwargs.iter();
                            for (field, value) in iter {
                                let field = field.extract::<String>()?;
                                match field.as_str() {
                                    #(stringify!(#fnames) => {
                                        ret.#fnames = value.extract()?;
                                    }),*
                                    _ => { return Err(pyo3::exceptions::PyKeyError::new_err(format!("unknown field {field}"))); }
                                }
                            }
                        }
                        Ok(ret)
                    }
                }
            });
        }
        syn::Fields::Unnamed(FieldsUnnamed { .. }) => {}
        syn::Fields::Unit => {
            panic!("PyStreamable does not support the unit type");
        }
    }

    py_protocol.extend(quote! {
        #[pyo3::pymethods]
        impl #ident {
            #[staticmethod]
            #[pyo3(signature=(json_dict))]
            pub fn from_json_dict(json_dict: &pyo3::PyAny) -> pyo3::PyResult<Self> {
                <Self as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(json_dict)
            }

            pub fn to_json_dict(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::PyObject> {
                #crate_name::to_json_dict::ToJsonDict::to_json_dict(self, py)
            }
        }
    });

    let streamable = quote! {
        #[pyo3::pymethods]
        impl #ident {
            #[staticmethod]
            #[pyo3(name = "from_bytes")]
            pub fn py_from_bytes(blob: pyo3::buffer::PyBuffer<u8>) -> pyo3::PyResult<Self> {
                if !blob.is_c_contiguous() {
                    panic!("from_bytes() must be called with a contiguous buffer");
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes())
                };
                <Self as #crate_name::Streamable>::from_bytes(slice).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e))
            }

            #[staticmethod]
            #[pyo3(name = "from_bytes_unchecked")]
            pub fn py_from_bytes_unchecked(blob: pyo3::buffer::PyBuffer<u8>) -> pyo3::PyResult<Self> {
                if !blob.is_c_contiguous() {
                    panic!("from_bytes_unchecked() must be called with a contiguous buffer");
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes())
                };
                <Self as #crate_name::Streamable>::from_bytes_unchecked(slice).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e))
            }

            // returns the type as well as the number of bytes read from the buffer
            #[staticmethod]
            #[pyo3(signature= (blob, trusted=false))]
            pub fn parse_rust<'p>(blob: pyo3::buffer::PyBuffer<u8>, trusted: bool) -> pyo3::PyResult<(Self, u32)> {
                if !blob.is_c_contiguous() {
                    panic!("parse_rust() must be called with a contiguous buffer");
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes())
                };
                let mut input = std::io::Cursor::<&[u8]>::new(slice);
                if trusted {
                    <Self as #crate_name::Streamable>::parse::<true>(&mut input).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e)).map(|v| (v, input.position() as u32))
                } else {
                    <Self as #crate_name::Streamable>::parse::<false>(&mut input).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e)).map(|v| (v, input.position() as u32))
                }
            }

            pub fn get_hash<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<&'p pyo3::types::PyBytes> {
                let mut ctx = <sha2::Sha256 as sha2::Digest>::new();
                #crate_name::Streamable::update_digest(self, &mut ctx);
                Ok(pyo3::types::PyBytes::new(py, sha2::Digest::finalize(ctx).as_slice()))
            }
            #[pyo3(name = "to_bytes")]
            pub fn py_to_bytes<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<&'p pyo3::types::PyBytes> {
                let mut writer = Vec::<u8>::new();
                #crate_name::Streamable::stream(self, &mut writer).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e))?;
                Ok(pyo3::types::PyBytes::new(py, &writer))
            }

            pub fn stream_to_bytes<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<&'p pyo3::types::PyBytes> {
                self.py_to_bytes(py)
            }

            pub fn __bytes__<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<&'p pyo3::types::PyBytes> {
                self.py_to_bytes(py)
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

#[proc_macro_derive(PyJsonDict)]
pub fn py_json_dict_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
                        <Self as #crate_name::Streamable>::parse::<false>(&mut std::io::Cursor::<&[u8]>::new(&[v])).map_err(|e| e.into())
                    }
                }
            }
            .into();
        }
        _ => {
            panic!("PyJsonDict only support struct");
        }
    };

    let mut py_protocol = quote! {};

    match fields {
        syn::Fields::Named(FieldsNamed { named, .. }) => {
            let mut fnames = Vec::<syn::Ident>::new();
            let mut ftypes = Vec::<syn::Type>::new();
            for f in named.iter() {
                fnames.push(f.ident.as_ref().unwrap().clone());
                ftypes.push(f.ty.clone());
            }

            py_protocol.extend( quote! {

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
        _ => {
            panic!("PyJsonDict only supports structs");
        }
    }

    py_protocol.into()
}

#[proc_macro_derive(PyGetters)]
pub fn py_getters_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let syn::Data::Struct(s) = data else {
        panic!("python binding only support struct");
    };

    let syn::Fields::Named(FieldsNamed { named, .. }) = s.fields else {
        panic!("python binding only support struct");
    };

    let found_crate = crate_name("chia-traits").expect("chia-traits is present in `Cargo.toml`");

    let crate_name = match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(#ident)
        }
    };

    let mut fnames = Vec::<syn::Ident>::new();
    let mut ftypes = Vec::<syn::Type>::new();
    for f in named.into_iter() {
        fnames.push(f.ident.unwrap());
        ftypes.push(f.ty);
    }

    let ret = quote! {
        #[pyo3::pymethods]
        impl #ident {
            #(
            #[getter]
            fn #fnames<'a> (&self, py: pyo3::Python<'a>) -> pyo3::PyResult<&'a pyo3::PyAny> {
                #crate_name::ChiaToPython::to_python(&self.#fnames, py)
            }
            )*
        }
    };

    ret.into()
}
