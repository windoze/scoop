//! `.slib` v1: deterministic archive packaging, typed member directory
//! and purpose-tiered artifact validation
//! (`docs/milestone23/DESIGN.md` sections 4.1-4.2).
//!
//! The container is a canonical SysV `ar` archive whose semantics live
//! entirely in the typed manifest directory; physical names, extensions
//! and order carry no meaning of their own.

pub mod archive;
pub mod artifact;
pub mod closure;
pub mod limits;
pub mod manifest;
pub mod member;
pub mod purpose {
    pub use crate::artifact::purpose::*;
}

#[cfg(test)]
mod closure_tests;
#[cfg(test)]
mod tests;

pub use artifact::{
    DecodedSlibEnvelope, ManifestCoreTemplate, SlibBuilder, SlibError, ValidatedGraphArtifact,
    ordinal_name,
};
pub use limits::SlibDecodeLimits;
pub use manifest::{ArtifactFingerprint, DependencyRecord, ManifestCore, ManifestError};
pub use member::{
    LogicalKey, MemberError, MemberPurposeSet, MemberStableKey, SlibMemberId, SlibMemberRecord,
    SlibMemberRole,
};
