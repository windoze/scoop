mod byte32_index;
mod decode;
mod encode;
pub use byte32_index::Byte32Index;

#[cfg(test)]
mod property_tests;

pub use decode::{
    BorrowedWireDecode, Decoder, WireDecode, decode_canonical, decode_canonical_borrowed,
};
pub(crate) use encode::encode_into_hasher;
pub use encode::{
    EncodeError, Encoder, WireEncode, encode, encode_canonical_temporary, encoded_length,
};
