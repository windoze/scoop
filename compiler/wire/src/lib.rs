//! Canonical wire primitives shared by persistent identity and `.slib`.
//!
//! This crate deliberately knows nothing about compiler entities. It provides
//! the restricted Wire CBOR v1 codec, domain-separated SHA-256 framing, and the
//! deterministic decode budget used by semantic readers.

pub mod budget;
pub mod cbor;
pub mod digest;
pub mod error;
pub mod path;

pub use budget::{BudgetMeter, DecodeLimitsV1, DecodeUsage, ResourceKind};
pub use cbor::{Decoder, Encoder, WireDecodeV1, WireEncodeV1, decode_canonical, encode};
pub use digest::{Digest256, HashError, byte_span, domain_separated_cbor_hash, sha256};
pub use error::{WireError, WireErrorKind, WireType};
pub use path::{PathSegment, WirePath};
