//! Canonical dependency keys for the complete cross-Cone layout/ABI section.
//!
//! These transport values identify semantic records. They do not grant
//! selected-use or machine-import authority by themselves.

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedStrongCallableDefinitionOwner,
    IdentityReferenceError, PersistentDispatchTableId, PersistentExactTypeId, PersistentLayoutId,
    PersistentTypeId, StrongCallableDefinitionOwner, ValidatedIdentityGraph,
};
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath,
};

mod dependency;
mod exports;
mod section;
mod semantic_closure;
mod target;
mod wire;

pub use dependency::{DecodedLayoutAbiDependencyV1, LayoutAbiDependencyV1};
pub use exports::{
    LayoutAbiExportConstituentsError, LayoutAbiExportConstituentsV1, LayoutAbiSemanticRecordV1,
};
pub use section::*;
pub use target::{DecodedLayoutAbiSemanticTargetV1, LayoutAbiSemanticTargetV1};

#[derive(Debug)]
pub enum LayoutAbiDependencyError {
    Identity(IdentityReferenceError),
    Resource(WireError),
}

impl From<IdentityReferenceError> for LayoutAbiDependencyError {
    fn from(error: IdentityReferenceError) -> Self {
        Self::Identity(error)
    }
}

impl From<WireError> for LayoutAbiDependencyError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for LayoutAbiDependencyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid layout/ABI dependency key: {self:?}")
    }
}

impl std::error::Error for LayoutAbiDependencyError {}

#[derive(Debug)]
pub enum LayoutAbiSemanticClosureError {
    ArithmeticOverflow,
    DuplicateTarget(LayoutAbiSemanticTargetV1),
    MissingTarget(LayoutAbiSemanticTargetV1),
    Provider {
        target: LayoutAbiSemanticTargetV1,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    EmbeddedRecord(LayoutAbiSemanticTargetV1),
    CurrentProvider(LayoutAbiSemanticTargetV1),
    NonCanonicalRoots,
    Hash(scoop_wire::HashError),
    Resource(WireError),
}

impl From<scoop_wire::HashError> for LayoutAbiSemanticClosureError {
    fn from(error: scoop_wire::HashError) -> Self {
        Self::Hash(error)
    }
}

impl From<WireError> for LayoutAbiSemanticClosureError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
