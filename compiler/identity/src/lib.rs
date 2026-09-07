//! Shared identity primitives for the Scoop toolchain: canonical CBOR,
//! domain-separated SHA-256 digests, Cone coordinates/identities, and
//! versioned capability ids.
//!
//! Every cross-artifact identity in milestone 23
//! (`docs/milestone23/DESIGN.md` sections 1.1 and 4.1) is derived from
//! canonical bytes, so the codec here is intentionally stricter than a
//! generic CBOR implementation: shortest integers, definite lengths,
//! integer-keyed maps with strictly ascending keys, no floats, tags or
//! indefinite items. Decoding any non-canonical spelling is an error,
//! never a silent reinterpretation.

pub mod capability;
pub mod cbor;
pub mod coordinate;
pub mod digest;
pub mod persistent;

pub use capability::{CapabilityError, CapabilityId, ObjectFormatId, TargetProfileWireId};
pub use cbor::{CborError, CborReader, CborWriter, MapGuard, SeqGuard};
pub use coordinate::{ConeCoordinate, ConeIdentity, CoordinateError, Version};
pub use digest::{Digest256, DomainHasher};
pub use persistent::{
    DefinitionKey, GeneratedRole, OwnerKind, OwnerStep, SymbolKind, mangle, truncated_runtime_id,
};

#[cfg(test)]
mod tests;

/// SHA-256 domain tag for `ConeIdentity` (DESIGN section 1.1).
pub const CONE_IDENTITY_DOMAIN: &[u8] = b"scoop-cone-id-v1";

/// Default nesting budget shared by hand-written recursive decoders
/// (DESIGN section 4.5 fixes 128 as the v1 CBOR nesting limit).
pub const CBOR_NESTING_LIMIT: u32 = 128;
