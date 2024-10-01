#![allow(clippy::missing_panics_doc)]

use proc_macro2::{Ident, Span};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{parse_macro_input, DeriveInput, FieldsNamed, FieldsUnnamed};

fn maybe_upper_fields(py_uppercase: bool, fnames: Vec<Ident>) -> Vec<Ident> {
    if py_uppercase {
        fnames
            .into_iter()
            .map(|f| Ident::new(&f.to_string().to_uppercase(), Span::call_site()))
            .collect()
    } else {
        fnames
    }
}

#[proc_macro_derive(PyStreamable, attributes(py_uppercase, py_pickle))]
pub fn py_streamable_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let found_crate = crate_name("chia-traits").expect("chia-traits is present in `Cargo.toml`");

    let crate_name = match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(#ident)
        }
    };

    let DeriveInput {
        ident, data, attrs, ..
    } = parse_macro_input!(input);

    let mut py_uppercase = false;
    let mut py_pickle = false;
    for attr in &attrs {
        if attr.path().is_ident("py_uppercase") {
            py_uppercase = true;
        } else if attr.path().is_ident("py_pickle") {
            py_pickle = true;
        }
    }

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
        syn::Data::Union(_) => {
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
            fn to_python<'a>(&self, py: pyo3::Python<'a>) -> pyo3::PyResult<pyo3::Bound<'a, pyo3::PyAny>> {
                Ok(pyo3::IntoPy::into_py(self.clone(), py).into_bound(py))
            }
        }
    };

    let mut fnames = Vec::<Ident>::new();
    let mut ftypes = Vec::<syn::Type>::new();

    match fields {
        syn::Fields::Named(FieldsNamed { named, .. }) => {
            for f in &named {
                fnames.push(f.ident.as_ref().unwrap().clone());
                ftypes.push(f.ty.clone());
            }

            let fnames_maybe_upper = maybe_upper_fields(py_uppercase, fnames.clone());

            py_protocol.extend(quote! {
                #[pyo3::pymethods]
                impl #ident {
                    #[allow(too_many_arguments)]
                    #[new]
                    #[pyo3(signature = (#(#fnames_maybe_upper),*))]
                    pub fn py_new ( #(#fnames_maybe_upper : #ftypes),* ) -> Self {
                        Self { #(#fnames: #fnames_maybe_upper),* }
                    }
                }
            });

            if !named.is_empty() {
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
                                        #(stringify!(#fnames_maybe_upper) => {
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
        }
        syn::Fields::Unnamed(FieldsUnnamed { .. }) => {}
        syn::Fields::Unit => {
            panic!("PyStreamable does not support the unit type");
        }
    }

    py_protocol.extend(quote! {
        #[pyo3::pymethods]
        impl #ident {
            #[classmethod]
            #[pyo3(signature=(json_dict))]
            pub fn from_json_dict(cls: &pyo3::Bound<'_, pyo3::types::PyType>, py: pyo3::Python<'_>, json_dict: &pyo3::Bound<pyo3::PyAny>) -> pyo3::PyResult<pyo3::PyObject> {
                use pyo3::prelude::PyAnyMethods;
                use pyo3::IntoPy;
                use pyo3::Bound;
                use pyo3::type_object::PyTypeInfo;
                use std::borrow::Borrow;
                let rust_obj = Bound::new(py, <Self as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(json_dict)?)?;

                // Check if python class is different from rust class (in case of child classes)
                // if so call the python class's conversion code

                if rust_obj.is_exact_instance(&cls) {
                    Ok(rust_obj.into_py(py))
                } else {
                    let instance = cls.call_method1("from_parent", (rust_obj.into_py(py),))?;
                    Ok(instance.into_py(py))
                }
            }

            pub fn to_json_dict(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::PyObject> {
                #crate_name::to_json_dict::ToJsonDict::to_json_dict(self, py)
            }
        }
    });

    let streamable = quote! {
        #[pyo3::pymethods]
        impl #ident {
            #[classmethod]
            #[pyo3(name = "from_bytes")]
            pub fn py_from_bytes(cls: &pyo3::Bound<'_, pyo3::types::PyType>, py: pyo3::Python<'_>, blob: pyo3::buffer::PyBuffer<u8>) -> pyo3::PyResult<pyo3::PyObject> {
                use pyo3::prelude::PyAnyMethods;
                use pyo3::IntoPy;
                use pyo3::Bound;
                use pyo3::type_object::PyTypeInfo;
                use std::borrow::Borrow;

                if !blob.is_c_contiguous() {
                    panic!("from_bytes() must be called with a contiguous buffer");
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes())
                };
                let rust_obj = Bound::new(py, <Self as #crate_name::Streamable>::from_bytes(slice)?)?;

                // Check if python class is different from rust class (in case of child classes)
                // if so call the python class's conversion code

                if rust_obj.is_exact_instance(&cls) {
                    Ok(rust_obj.into_py(py))
                } else {
                    let instance = cls.call_method1("from_parent", (rust_obj.into_py(py),))?;
                    Ok(instance.into_py(py))
                }
            }

            #[classmethod]
            #[pyo3(name = "from_bytes_unchecked")]
            pub fn py_from_bytes_unchecked(cls: &pyo3::Bound<'_, pyo3::types::PyType>, py: pyo3::Python<'_>, blob: pyo3::buffer::PyBuffer<u8>) -> pyo3::PyResult<pyo3::PyObject> {
                use pyo3::prelude::PyAnyMethods;
                use pyo3::IntoPy;
                use pyo3::Bound;
                use pyo3::type_object::PyTypeInfo;
                use std::borrow::Borrow;
                if !blob.is_c_contiguous() {
                    panic!("from_bytes_unchecked() must be called with a contiguous buffer");
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes())
                };
                let rust_obj = Bound::new(py, <Self as #crate_name::Streamable>::from_bytes_unchecked(slice).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e))?)?;

                // Check if python class is different from rust class (in case of child classes)
                // if so call the python class's conversion code

                if rust_obj.is_exact_instance(&cls) {
                    Ok(rust_obj.into_py(py))
                } else {
                    let instance = cls.call_method1("from_parent", (rust_obj.into_py(py),))?;
                    Ok(instance.into_py(py))
                }
            }

            // returns the type as well as the number of bytes read from the buffer
            #[classmethod]
            #[pyo3(signature= (blob, trusted=false))]
            pub fn parse_rust<'p>(cls: &pyo3::Bound<'_, pyo3::types::PyType>, py: pyo3::Python<'_>, blob: pyo3::buffer::PyBuffer<u8>, trusted: bool) -> pyo3::PyResult<(pyo3::PyObject, u32)> {
                use pyo3::prelude::PyAnyMethods;
                use pyo3::IntoPy;
                use pyo3::Bound;
                use pyo3::type_object::PyTypeInfo;
                use std::borrow::Borrow;
                if !blob.is_c_contiguous() {
                    panic!("parse_rust() must be called with a contiguous buffer");
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes())
                };
                let mut input = std::io::Cursor::<&[u8]>::new(slice);
                let rust_obj = if trusted {
                    <Self as #crate_name::Streamable>::parse::<true>(&mut input).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e)).map(|v| (v, input.position() as u32))
                } else {
                    <Self as #crate_name::Streamable>::parse::<false>(&mut input).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e)).map(|v| (v, input.position() as u32))
                }?;

                // Check if python class is different from rust class (in case of child classes)
                // if so call the python class's conversion code

                let rust_obj = (Bound::new(py, rust_obj.0)?, rust_obj.1);

                if rust_obj.0.is_exact_instance(&cls) {
                    Ok((rust_obj.0.into_py(py), rust_obj.1))
                } else {
                    let instance = cls.call_method1("from_parent", (rust_obj.0.into_py(py),))?;
                    Ok((instance.into_py(py), rust_obj.1))
                }
            }

            pub fn get_hash<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
                use pyo3::IntoPy;
                use pyo3::types::PyModule;
                use pyo3::prelude::PyAnyMethods;
                let mut ctx = chia_sha2::Sha256::new();
                #crate_name::Streamable::update_digest(self, &mut ctx);
                let bytes_module = PyModule::import_bound(py, "chia_rs.sized_bytes")?;
                let ty = bytes_module.getattr("bytes32")?;
                ty.call1((&ctx.finalize().into_py(py),))
            }
            #[pyo3(name = "to_bytes")]
            pub fn py_to_bytes<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<pyo3::Bound<'p, pyo3::types::PyBytes>> {
                let mut writer = Vec::<u8>::new();
                #crate_name::Streamable::stream(self, &mut writer).map_err(|e| <#crate_name::chia_error::Error as Into<pyo3::PyErr>>::into(e))?;
                Ok(pyo3::types::PyBytes::new_bound(py, &writer))
            }

            pub fn stream_to_bytes<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<pyo3::Bound<'p, pyo3::types::PyBytes>> {
                self.py_to_bytes(py)
            }

            pub fn __bytes__<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<pyo3::Bound<'p, pyo3::types::PyBytes>> {
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

    if py_pickle {
        let pickle = quote! {
            #[pyo3::pymethods]
            impl #ident {
                pub fn __setstate__(
                    &mut self,
                    state: &pyo3::types::PyBytes,
                ) -> pyo3::PyResult<()> {
                    use chia_traits::Streamable;

                    *self = Self::parse::<true>(&mut std::io::Cursor::new(state.as_bytes()))?;

                    Ok(())
                }

                pub fn __getstate__<'py>(
                    &self,
                    py: pyo3::Python<'py>,
                ) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyBytes>> {
                    self.py_to_bytes(py)
                }

                pub fn __getnewargs__<'py>(&self, py: pyo3::Python<'py>) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyTuple>> {
                    let mut args = Vec::new();
                    #( args.push(#crate_name::ChiaToPython::to_python(&self.#fnames, py)?); )*
                    Ok(pyo3::types::PyTuple::new_bound(py, args))
                }
            }
        };
        py_protocol.extend(pickle);
    }

    py_protocol.into()
}

#[proc_macro_derive(PyJsonDict, attributes(py_uppercase))]
pub fn py_json_dict_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let found_crate = crate_name("chia-traits").expect("chia-traits is present in `Cargo.toml`");

    let crate_name = match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(#ident)
        }
    };

    let DeriveInput {
        ident, data, attrs, ..
    } = parse_macro_input!(input);

    let mut py_uppercase = false;
    for attr in &attrs {
        if attr.path().is_ident("py_uppercase") {
            py_uppercase = true;
        }
    }

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
                    fn from_json_dict(o: &pyo3::Bound<pyo3::PyAny>) -> pyo3::PyResult<Self> {
                        let v = <u8 as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(o)?;
                        <Self as #crate_name::Streamable>::parse::<false>(&mut std::io::Cursor::<&[u8]>::new(&[v])).map_err(|e| e.into())
                    }
                }
            }
            .into();
        }
        syn::Data::Union(_) => {
            panic!("PyJsonDict only support struct");
        }
    };

    let mut py_protocol = quote! {};

    match fields {
        syn::Fields::Named(FieldsNamed { named, .. }) => {
            let mut fnames = Vec::<Ident>::new();
            let mut ftypes = Vec::<syn::Type>::new();
            for f in &named {
                fnames.push(f.ident.as_ref().unwrap().clone());
                ftypes.push(f.ty.clone());
            }

            let fnames_maybe_upper = maybe_upper_fields(py_uppercase, fnames.clone());

            py_protocol.extend( quote! {

                impl #crate_name::to_json_dict::ToJsonDict for #ident {
                    fn to_json_dict(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::PyObject> {
                        use pyo3::prelude::PyDictMethods;
                        let ret = pyo3::types::PyDict::new_bound(py);
                        #(ret.set_item(stringify!(#fnames_maybe_upper), self.#fnames.to_json_dict(py)?)?);*;
                        Ok(ret.into())
                    }
                }

                impl #crate_name::from_json_dict::FromJsonDict for #ident {
                    fn from_json_dict(o: &pyo3::Bound<pyo3::PyAny>) -> pyo3::PyResult<Self> {
                        use pyo3::prelude::PyAnyMethods;
                        Ok(Self{
                            #(#fnames: <#ftypes as #crate_name::from_json_dict::FromJsonDict>::from_json_dict(&o.get_item(stringify!(#fnames_maybe_upper))?)?,)*
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

#[proc_macro_derive(PyGetters, attributes(py_uppercase))]
pub fn py_getters_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let DeriveInput {
        ident, data, attrs, ..
    } = parse_macro_input!(input);

    let mut py_uppercase = false;
    for attr in &attrs {
        if attr.path().is_ident("py_uppercase") {
            py_uppercase = true;
        }
    }

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

    let mut fnames = Vec::<Ident>::new();
    let mut ftypes = Vec::<syn::Type>::new();
    for f in named {
        fnames.push(f.ident.unwrap());
        ftypes.push(f.ty);
    }

    let fnames_maybe_upper = maybe_upper_fields(py_uppercase, fnames.clone());

    let ret = quote! {
        #[pyo3::pymethods]
        impl #ident {
            #(
            #[getter]
            fn #fnames_maybe_upper<'a> (&self, py: pyo3::Python<'a>) -> pyo3::PyResult<pyo3::Bound<'a, pyo3::PyAny>> {
                #crate_name::ChiaToPython::to_python(&self.#fnames, py)
            }
            )*
        }
    };

    ret.into()
}
