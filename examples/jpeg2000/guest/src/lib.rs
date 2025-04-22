#![cfg_attr(feature = "guest", no_std)]

use serde::{Deserialize, Serialize};
//#[macro_use]
//extern crate serde_big_array;

use core::fmt;
use serde::{
    de::{SeqAccess, Visitor},
    ser::{SerializeSeq, Serializer},
    Deserializer,
};

const N: usize = 1000;
#[derive(Clone)]
pub struct MyArray([u8; N]);

impl Default for MyArray {
    fn default() -> Self {
        MyArray([0u8; 1000])
    }
}

impl Serialize for MyArray {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(N))?;
        for element in self.0.iter() {
            seq.serialize_element(element)?;
        }
        seq.end()
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
                formatter.write_str("an array")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<MyArray, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut arr = [0; N];
                for i in 0..N {
                    arr[i] = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                Ok(MyArray(arr))
            }
        }

        deserializer.deserialize_seq(ArrayVisitor)
    }
}

#[jolt::provable]
fn collatz_convergence_range(start: u128, end: u128) -> u128 {
    let mut max_num_steps = 0;
    for n in start..end {
        let num_steps = collatz_convergence(n);
        if num_steps > max_num_steps {
            max_num_steps = num_steps;
        }
    }
    max_num_steps
}

#[jolt::provable]
fn collatz_convergence(n: u128) -> u128 {
    let mut n = n;
    let mut num_steps = 0;
    while n != 1 {
        if n % 2 == 0 {
            n /= 2;
        } else {
            n += (n << 1) + 1;
        }
        num_steps += 1;
    }
    return num_steps;
}

// #[jolt::provable(max_input_size = 10000, max_output_size = 10000)]

// Let define a type for the buffer of u8 containing images, with the size. The type has to be serializable.
// The type should just be a rename of the array:
//#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
//pub struct ImageBuffer(#[serde(with = "BigArray")] pub [u8; 1000]);

impl MyArray {
    pub fn new(data: &[u8]) -> Self {
        let mut buffer = [0u8; N];
        buffer.copy_from_slice(data);
        MyArray(buffer)
    }
}

#[jolt::provable]
pub fn jpeg2000(data: MyArray) -> bool {
    return validate_jpeg2k(data.0.as_ref());
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
