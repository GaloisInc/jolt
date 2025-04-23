#![cfg_attr(feature = "guest", no_std)]

use serde::{Deserialize, Serialize};
//#[macro_use]
//extern crate serde_big_array;

use core::fmt;
use serde::{
    de::{SeqAccess, Visitor},
    ser::{SerializeTuple, Serializer},
    Deserializer,
};

pub const N: usize = 1000;
#[derive(Clone)]
pub struct MyArray([u8; N]);

impl Default for MyArray {
    fn default() -> Self {
        MyArray([0u8; N])
    }
}

impl Serialize for MyArray {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(N)?;
        for &b in &self.0 {
            tup.serialize_element(&b)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for MyArray {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor;

        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = MyArray;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an array of length N")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<MyArray, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut arr = [0u8; N];
                for i in 0..N {
                    arr[i] = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                // ensure there are no extra elements
                if let Some(_) = seq.next_element::<u8>()? {
                    return Err(serde::de::Error::invalid_length(N + 1, &self));
                }
                Ok(MyArray(arr))
            }
        }

        deserializer.deserialize_tuple(N, ArrayVisitor)
    }
}

// #[jolt::provable(max_input_size = 10000, max_output_size = 10000)]

// Let define a type for the buffer of u8 containing images, with the size. The type has to be serializable.
// The type should just be a rename of the array:
//#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
//pub struct ImageBuffer(#[serde(with = "BigArray")] pub [u8; 1000]);

impl MyArray {
    pub fn new(data: &[u8]) -> Self {
        let mut buffer = [0u8; N];
        let len = data.len();
        if len > N {
            panic!("Data is too long");
        }
        for i in 0..len {
            buffer[i] = data[i];
        }
        for i in len..N {
            buffer[i] = 0;
        }

        MyArray(buffer)
    }
}

#[jolt::provable]
pub fn jpeg2000(data: MyArray, len: usize) -> bool {
    // Get a slice of MyArray of size len
    let data_slice = &data.0[0..len];

    return validate_jpeg2k(data_slice);
}

pub fn validate_jpeg2k(data: &[u8]) -> bool {
    let mut pos: usize = 0; // Current index in the image
    let mut res: bool;
    /* Signature Box */
    // The signature box size is 12 bytes long [0x 0000 000C 6A50 0D0A 870A]
    let signature_box_size: [u8; 4] = [0x00, 0x00, 0x00, 0x0C];
    let signature_box_content: [u8; 8] = [0x6A, 0x50, 0x20, 0x20, 0x0D, 0x0A, 0x87, 0x0A];
    // Box size
    if data[pos..pos + 4] != signature_box_size {
        return false;
    }
    if data[pos + 4..pos + 12] != signature_box_content {
        return false;
    }

    return true;
}
