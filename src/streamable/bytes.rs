use core::fmt::Formatter;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize)]
pub struct Bytes4([u8; 4]);

impl Debug for Bytes4 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        formatter.write_str(&hex::encode(self.0))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Bytes32([u8; 32]);

impl Debug for Bytes32 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        formatter.write_str(&hex::encode(self.0))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Bytes48(Bytes32, [u8; 16]);

impl Debug for Bytes48 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.0.fmt(formatter)?;
        formatter.write_str(&hex::encode(self.1))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Bytes96(Bytes32, Bytes32, Bytes32);

impl Debug for Bytes96 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.0.fmt(formatter)?;
        self.1.fmt(formatter)?;
        self.2.fmt(formatter)
    }
}

// TODO: this is a hack to eliminate the need to serialize manually
#[derive(Serialize, Deserialize)]
pub struct Bytes100(Bytes96, Bytes4);

impl Debug for Bytes100 {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.0.fmt(formatter)?;
        self.1.fmt(formatter)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Bytes(Vec<u8>);

impl Debug for Bytes {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        formatter.write_str(&hex::encode(&self.0))
    }
}
