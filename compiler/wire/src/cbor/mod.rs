mod decode;
mod encode;

#[cfg(test)]
mod property_tests;

pub use decode::{
    BorrowedWireDecode, Decoder, WireDecode, decode_canonical, decode_canonical_borrowed,
};
pub(crate) use encode::encode_into_hasher;
pub use encode::{
    EncodeError, Encoder, WireEncode, encode, encode_canonical_temporary, encoded_length,
};
