use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeTupleStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fmt::Debug;

pub struct ProgramArray(pub Vec<u8>);

impl Debug for ProgramArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ProgramArray")
            .field(&hex::encode(&self.0))
            .finish()
    }
}
impl<'de> Deserialize<'de> for ProgramArray {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = ProgramArrayVisitor {};
        deserializer.deserialize_tuple(1024*1024*1024, visitor)
    }
}

fn size_for_initial_byte<'de, S>(
    initial_byte: u8,
    seq: &mut S,
    vec: &mut Vec<u8>,
) -> Result<usize, S::Error>
where
    S: SeqAccess<'de>,
{
    let mut size: usize = 0;
    if initial_byte & 0xc0 == 0x80 {
        size = (initial_byte & 0x7f) as usize;
    }
    if initial_byte & 0xe0 == 0xc0 {
        let size_byte: [u8; 1] = seq.next_element()?.unwrap();
        size = (((initial_byte as usize) & 0x3f) << 8) | (size_byte[0] as usize);
        vec.extend_from_slice(&size_byte);
    }
    if initial_byte & 0xf0 == 0xe0 {
        let size_byte: [u8; 2] = seq.next_element()?.unwrap();
        size = (((initial_byte as usize) & 0x1f) << 16)
            | ((size_byte[0] as usize) << 8)
            | (size_byte[1] as usize);
        vec.extend_from_slice(&size_byte);
    }

    Ok(size)
}

fn blob_for_size<'de, S>(mut size: usize, seq: &mut S, vec: &mut Vec<u8>) -> Result<(), S::Error>
where
    S: SeqAccess<'de>,
{
    vec.reserve(size);
    while size >= 32 {
        let b: [u8; 32] = seq.next_element()?.unwrap();
        vec.extend_from_slice(&b);
        size -= 32;
    }
    while size >= 8 {
        let b: [u8; 8] = seq.next_element()?.unwrap();
        vec.extend_from_slice(&b);
        size -= 8;
    }
    while size >= 1 {
        let b: [u8; 1] = seq.next_element()?.unwrap();
        vec.extend_from_slice(&b);
        size -= 1;
    }
    Ok(())
}

struct ProgramArrayVisitor {}
impl<'de> Visitor<'de> for ProgramArrayVisitor {
    type Value = ProgramArray;

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut node_count = 1;
        let mut r = Vec::with_capacity(256);
        while node_count > 0 {
            node_count -= 1;
            let initial_byte: u8 = seq.next_element()?.unwrap();
            r.push(initial_byte);
            if initial_byte == 0xff {
                node_count += 2;
            } else if initial_byte & 0x80 != 0 {
                let size = size_for_initial_byte(initial_byte, &mut seq, &mut r)?;
                blob_for_size(size, &mut seq, &mut r)?;
            }
        }
        r.shrink_to_fit();
        let r = ProgramArray(r);
        Ok(r)
    }

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "blob!!")
    }
}

impl Serialize for ProgramArray {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let r = &self.0;
        let len = r.len();
        let mut st = serializer.serialize_tuple_struct("Node", len)?;
        for c in r {
            st.serialize_field(&c)?;
        }
        st.end()
    }
}
