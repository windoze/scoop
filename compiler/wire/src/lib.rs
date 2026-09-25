//! Canonical wire primitives shared by persistent identity and `.slib`.
//!
//! This crate deliberately knows nothing about compiler entities. It provides
//! the restricted Wire CBOR v1 codec, domain-separated SHA-256 framing, and the
//! fallible allocation helpers used by metadata readers.

pub mod allocation;
pub mod cbor;
pub mod digest;
pub mod error;
pub mod path;
pub mod runtime;

pub use cbor::{
    BorrowedWireDecode, Decoder, Encoder, WireDecode, WireEncode, decode_canonical,
    decode_canonical_borrowed, encode, encode_canonical_temporary, encoded_length,
};
pub use digest::{
    CanonicalHashStream, Digest256, HashError, byte_span, domain_separated_cbor_hash,
    domain_separated_raw_hash, domain_separated_runtime_hash, sha256,
};
pub use error::{WireError, WireErrorKind, WireType};
pub use path::{PathSegment, WirePath};
pub use runtime::{
    RuntimeDecode, RuntimeDecodeError, RuntimeDecodeErrorKind, RuntimeDecoder, RuntimeEncode,
    RuntimeEncodeError, RuntimeEncoder, decode_runtime, encode_runtime,
};
