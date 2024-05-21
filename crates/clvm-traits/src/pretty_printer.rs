use num_bigint::BigInt;

use crate::{ClvmEncoder, ToClvm};

const PRINTABLE: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[]^_\\`{|}~ \t\r\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrettyPrinter {
    pub max_int_bytes: usize,
    pub atom_strings: bool,
}

impl Default for PrettyPrinter {
    fn default() -> Self {
        Self {
            max_int_bytes: 2,
            atom_strings: true,
        }
    }
}

pub trait PrettyPrint {
    fn pretty_print(&self) -> String;
}

impl<T> PrettyPrint for T
where
    T: ToClvm<Pretty>,
{
    fn pretty_print(&self) -> String {
        self.to_clvm(&mut PrettyPrinter::default()).unwrap().0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pretty(pub String);

impl ClvmEncoder for PrettyPrinter {
    type Node = Pretty;

    fn encode_atom(&mut self, bytes: &[u8]) -> Result<Self::Node, crate::ToClvmError> {
        if bytes.is_empty() {
            return Ok(Pretty("()".to_string()));
        }

        if bytes.len() <= self.max_int_bytes {
            let int = BigInt::from_signed_bytes_be(bytes);

            if int.to_signed_bytes_be() != bytes {
                return Ok(Pretty(format!("0x{}", hex::encode(bytes))));
            }

            return Ok(Pretty(BigInt::from_signed_bytes_be(bytes).to_string()));
        }

        if !self.atom_strings {
            return Ok(Pretty(format!("0x{}", hex::encode(bytes))));
        }

        let text = String::from_utf8_lossy(bytes);

        if text.chars().any(|c| !PRINTABLE.contains(c)) {
            return Ok(Pretty(format!("0x{}", hex::encode(bytes))));
        }

        if text.contains('"') && text.contains('\'') {
            return Ok(Pretty(format!("0x{}", hex::encode(bytes))));
        }

        let quote = if text.contains('"') { "'" } else { "\"" };

        Ok(Pretty(format!("{}{}{}", quote, text, quote)))
    }

    fn encode_pair(
        &mut self,
        first: Self::Node,
        rest: Self::Node,
    ) -> Result<Self::Node, crate::ToClvmError> {
        if rest.0.as_str() == "()" {
            return Ok(Pretty(format!("({})", first.0)));
        }

        if let Some(rest) = rest.0.strip_prefix('(') {
            debug_assert!(rest.ends_with(')'));
            return Ok(Pretty(format!("({} {}", first.0, rest)));
        }

        Ok(Pretty(format!("({} . {})", first.0, rest.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atom() {
        assert_eq!(4.pretty_print(), "4");
        assert_eq!(0x4324284700u64.pretty_print(), "0x4324284700");

        assert_eq!("Hello, world!".pretty_print(), "\"Hello, world!\"");
        assert_eq!(
            "Hello, world!"
                .to_clvm(&mut PrettyPrinter {
                    atom_strings: false,
                    ..Default::default()
                })
                .unwrap()
                .0,
            "0x48656c6c6f2c20776f726c6421"
        );
    }

    #[test]
    fn test_pair() {
        assert_eq!((1, 2).pretty_print(), "(1 . 2)");
        assert_eq!((1, (2, 3)).pretty_print(), "(1 2 . 3)");
        assert_eq!([1, 2, 3, 4].pretty_print(), "(1 2 3 4)");
    }
}
