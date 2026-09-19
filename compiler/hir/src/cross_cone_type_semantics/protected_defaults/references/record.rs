use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentObjectValueId, PersistentPropertyId,
    SignatureTypeKey,
};
use scoop_wire::{Encoder, WireEncode};

use super::super::{CanonicalProtectedDefaultExpressionUsesV1, ProtectedDefaultAccessWitnessV1};
use super::DecodedProtectedDefaultReferenceV1;
use crate::{
    DecodedDefaultConstructorRefV1, DecodedDefaultFieldRefV1, DecodedExportDefaultCallableTargetV1,
    DefaultConstructorRefV1, DefaultFieldRefV1, ExportDefaultCallableTargetV1,
    ExportDefinitionSourceV1,
};

/// A transported reference and its complete expression-use list, without access authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedDefaultReferenceV1<T> {
    target: T,
    definition_origin: ExportDefinitionSourceV1,
    witness: ProtectedDefaultAccessWitnessV1,
    uses: CanonicalProtectedDefaultExpressionUsesV1,
}

impl<T> ProtectedDefaultReferenceV1<T> {
    pub const fn new(
        target: T,
        definition_origin: ExportDefinitionSourceV1,
        witness: ProtectedDefaultAccessWitnessV1,
        uses: CanonicalProtectedDefaultExpressionUsesV1,
    ) -> Self {
        Self {
            target,
            definition_origin,
            witness,
            uses,
        }
    }

    pub const fn target(&self) -> &T {
        &self.target
    }
    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
    pub const fn witness(&self) -> &ProtectedDefaultAccessWitnessV1 {
        &self.witness
    }
    pub const fn uses(&self) -> &CanonicalProtectedDefaultExpressionUsesV1 {
        &self.uses
    }
}

impl<T: WireEncode> WireEncode for ProtectedDefaultReferenceV1<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.definition_origin.encode(encoder)?;
        encoder.field(3)?;
        self.witness.encode(encoder)?;
        encoder.field(4)?;
        self.uses.encode(encoder)
    }
}

pub type ProtectedDefaultCallableReferenceV1 =
    ProtectedDefaultReferenceV1<ExportDefaultCallableTargetV1>;
pub type ProtectedDefaultConstructorReferenceV1 =
    ProtectedDefaultReferenceV1<DefaultConstructorRefV1>;
pub type ProtectedDefaultTypeReferenceV1 = ProtectedDefaultReferenceV1<SignatureTypeKey>;
pub type ProtectedDefaultGlobalReferenceV1 = ProtectedDefaultReferenceV1<PersistentPropertyId>;
pub type ProtectedDefaultSingletonReferenceV1 =
    ProtectedDefaultReferenceV1<PersistentObjectValueId>;
pub type ProtectedDefaultFieldReferenceV1 = ProtectedDefaultReferenceV1<DefaultFieldRefV1>;

pub type DecodedProtectedDefaultCallableReferenceV1 =
    DecodedProtectedDefaultReferenceV1<DecodedExportDefaultCallableTargetV1>;
pub type DecodedProtectedDefaultConstructorReferenceV1 =
    DecodedProtectedDefaultReferenceV1<DecodedDefaultConstructorRefV1>;
pub type DecodedProtectedDefaultTypeReferenceV1 =
    DecodedProtectedDefaultReferenceV1<DecodedSignatureTypeKey>;
pub type DecodedProtectedDefaultGlobalReferenceV1 =
    DecodedProtectedDefaultReferenceV1<DecodedPersistentId<PersistentPropertyId>>;
pub type DecodedProtectedDefaultSingletonReferenceV1 =
    DecodedProtectedDefaultReferenceV1<DecodedPersistentId<PersistentObjectValueId>>;
pub type DecodedProtectedDefaultFieldReferenceV1 =
    DecodedProtectedDefaultReferenceV1<DecodedDefaultFieldRefV1>;
