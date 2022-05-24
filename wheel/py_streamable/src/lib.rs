extern crate proc_macro;
#[macro_use]
extern crate quote;

use syn::{parse_macro_input, DeriveInput, FieldsNamed};

use proc_macro::TokenStream;

#[proc_macro_derive(Streamable)]
pub fn py_streamable_macro(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let mut py_protocol = quote! {
        #[pyproto]
        impl PyObjectProtocol for #ident {
            fn __str__(&self) -> PyResult<String> {
                Ok(format!("{:?}", self))
            }

            fn __repr__(&self) -> PyResult<String> {
                Ok(format!("{:?}", self))
            }

            fn __richcmp__(&self, other: PyRef<#ident>, op: CompareOp) -> Py<PyAny> {
                let py = other.py();
                match op {
                    CompareOp::Eq => (self == &*other).into_py(py),
                    CompareOp::Ne => (self != &*other).into_py(py),
                    _ => py.NotImplemented(),
                }
            }

            fn __hash__(&self) -> PyResult<isize> {
                let mut hasher = DefaultHasher::new();
                std::hash::Hash::hash(self, &mut hasher);
                Ok(hasher.finish() as isize)
            }
        }
    };
    let constructor = match data {
        syn::Data::Struct(s) => {
            let mut fnames = Vec::<syn::Ident>::new();
            let mut ftypes = Vec::<syn::Type>::new();
            match s.fields {
                syn::Fields::Named(FieldsNamed { named, .. }) => {
                    for f in named.iter() {
                        fnames.push(f.ident.as_ref().unwrap().clone());
                        ftypes.push(f.ty.clone());
                    }
                }
                _ => {
                    panic!("Streamable requires named fields");
                }
            }

            quote! {
                #[pymethods]
                impl #ident {
                    #[new]
                    fn new ( #(#fnames : #ftypes),* ) -> #ident {
                        #ident { #(#fnames),* }
                    }

                    #[staticmethod]
                    pub fn from_bytes(blob: &[u8]) -> PyResult<Self> {
                        let mut de = ChiaDeserializer::from_slice(blob)?;
                        Self::deserialize(&mut de).map_err(|e| e.into())
                    }

                    // returns the type as well as the number of bytes read from the buffer
                    #[staticmethod]
                    pub fn parse_rust(blob: &[u8]) -> PyResult<(Self, u32)> {
                        let mut de = ChiaDeserializer::from_slice(blob)?;
                        Self::deserialize(&mut de)
                            .map_err(|e| e.into())
                            .map(|v| (v, de.pos()))
                    }

                    pub fn to_bytes<'p>(&self, py: Python<'p>) -> PyResult<&'p PyBytes> {
                        let mut writer = Vec::<u8>::new();
                        let mut ser = ChiaSerializer::new(&mut writer);
                        serde::Serialize::serialize(self, &mut ser)?;
                        Ok(PyBytes::new(py, &writer))
                    }

                    pub fn __bytes__<'p>(&self, py: Python<'p>) -> PyResult<&'p PyBytes> {
                        self.to_bytes(py)
                    }

                    pub fn __deepcopy__<'p>(&self, memo: &PyAny) -> PyResult<Self> {
                        Ok(self.clone())
                    }

                    pub fn __copy__<'p>(&self) -> PyResult<Self> {
                        Ok(self.clone())
                    }

                    pub fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
                        ToJsonDict::to_json_dict(self, py)
                    }

                    #[staticmethod]
                    pub fn from_json_dict(o: &PyAny) -> PyResult<Self> {
                        Ok(<Self as FromJsonDict>::from_json_dict(o)?)
                    }
                }

                impl ToJsonDict for #ident {
                    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
                        let ret = PyDict::new(py);
                        #(ret.set_item(stringify!(#fnames), self.#fnames.to_json_dict(py)?)?);*;
                        Ok(ret.into())
                    }
                }


                impl FromJsonDict for #ident {
                    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
                        Ok(Self{
                            #(#fnames: <#ftypes as FromJsonDict>::from_json_dict(o.get_item(stringify!(#fnames))?)?,)*
                        })
                    }
                }

            }
        }
        _ => {
            panic!("Streamable only support struct");
        }
    };

    py_protocol.extend(constructor);
    py_protocol.into()
}
