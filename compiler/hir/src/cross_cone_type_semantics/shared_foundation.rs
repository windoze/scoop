//! Queries over saved declaration metadata used by compilation.

use scoop_identity::{
    ConeIdentity, PersistentExactTypeId, PersistentTypeId, ValidatedIdentityGraph,
};

use crate::{CrossConeHirInterfaceSectionV1, NominalMaterializationClosure};

mod applications;
mod companions;
mod equality_applications;
mod errors;
mod inheritance;
mod keys;
mod object_initialization;
mod sources;
mod type_uses;
use SharedTypeMetadataError as Error;
pub use errors::SharedTypeMetadataError;

/// The same decoded declaration data used by ordinary HIR readers. No source
/// transcript or compilation-process evidence is part of this input.
#[derive(Clone, Copy)]
pub struct SharedTypeMetadataV1<'a> {
    pub provider: ConeIdentity,
    pub identities: &'a ValidatedIdentityGraph,
    pub foundation: &'a crate::CanonicalHirFoundation,
    pub public: &'a CrossConeHirInterfaceSectionV1,
}

impl<'a> SharedTypeMetadataV1<'a> {
    /// Borrows the original source unit table without implying materialization.
    pub fn source_initialization_units(
        self,
    ) -> &'a [scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentInitializationUnitId,
        scoop_identity::InitializationUnitKey,
    >] {
        self.foundation.type_source_initialization_records()
    }

    /// Resolves a parameter-free source signature through this artifact's
    /// canonical identity graph, including each structural component.
    pub fn signature_exact_type(
        self,
        signature: &scoop_identity::SignatureTypeKey,
    ) -> Result<PersistentExactTypeId, SharedTypeMetadataError> {
        self.exact(signature)
    }

    pub fn signature_exact_type_with_bindings(
        self,
        signature: &scoop_identity::SignatureTypeKey,
        bindings: &[Vec<PersistentExactTypeId>],
        identities: &'a ValidatedIdentityGraph,
    ) -> Result<PersistentExactTypeId, SharedTypeMetadataError> {
        Self { identities, ..self }.exact_with_bindings(signature, bindings)
    }
}
