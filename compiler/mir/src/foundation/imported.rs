use std::fmt;

use scoop_identity::{
    CallableOwner, ConeIdentity, ExactCallableSignature, ImportedIdentityId, ImportedIdentityMap,
    MirIdentityLayer, PersistentFunctionId, PersistentId, StrongCallableDefinitionOwner,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalMirFoundation, MirFoundationCounts, OdrFreeMirFoundation, ValidatedMirFoundation,
};

/// Session-local MIR identity. It cannot be interchanged with imported HIR or
/// LIR identities even when the persistent kind is the same.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedMirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedMirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// The MIR identity foundation after atomic import into a semantic session.
pub struct ImportedMirFoundation {
    canonical: CanonicalMirFoundation,
    identities: ImportedIdentityMap<MirIdentityLayer>,
}

impl ImportedMirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedMirFoundation,
        identities: ImportedIdentityMap<MirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeMirFoundation,
        identities: ImportedIdentityMap<MirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> MirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedMirId<I>> {
        self.identities.get(id).map(ImportedMirId)
    }

    /// Projects the required core-internal initialization cycle service. Its
    /// typed role is disjoint from public prelude bindings.
    pub fn project_initialization_cycle_thrower(
        &self,
        production: &crate::CoreBootstrapBridgeSectionV1,
        definition: PersistentFunctionId,
        signature: ExactCallableSignature,
    ) -> Result<crate::SelectedDependencyMirCallableV1, ImportedMirCallableProjectionError> {
        if self.origin() != ConeIdentity::CORE {
            return Err(ImportedMirCallableProjectionError::FoundationNotCore(
                self.origin(),
            ));
        }
        let implementation = CallableOwner::Function(definition);
        let bridge = production
            .strong_callable_bridges()
            .get(implementation)
            .ok_or(ImportedMirCallableProjectionError::MissingStrongSignature(
                definition,
            ))?;
        if bridge.role() != crate::CallableRole::InitializationCycle {
            return Err(
                ImportedMirCallableProjectionError::InitializationCycleRoleMismatch(definition),
            );
        }
        self.project_checked_callable(production, definition, implementation, signature)
    }

    fn project_checked_callable(
        &self,
        production: &crate::CoreBootstrapBridgeSectionV1,
        definition: PersistentFunctionId,
        implementation: CallableOwner,
        signature: ExactCallableSignature,
    ) -> Result<crate::SelectedDependencyMirCallableV1, ImportedMirCallableProjectionError> {
        let strong_bridge = production
            .strong_callable_bridges()
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == implementation)
            .ok_or(ImportedMirCallableProjectionError::MissingStrongSignature(
                definition,
            ))?;
        if strong_bridge.signature() != &signature {
            return Err(ImportedMirCallableProjectionError::StrongSignatureMismatch(
                definition,
            ));
        }
        let foundation_signature = self
            .canonical
            .callable_signatures()
            .iter()
            .find(|candidate| {
                candidate.subject() == crate::CallableSignatureSubject::Strong(implementation)
            })
            .ok_or(ImportedMirCallableProjectionError::MissingStrongSignature(
                definition,
            ))?;
        if foundation_signature.signature() != &signature {
            return Err(ImportedMirCallableProjectionError::StrongSignatureMismatch(
                definition,
            ));
        }
        crate::SelectedDependencyMirCallableV1::try_new(
            self.origin(),
            scoop_identity::DependencyCallableDeclarationId::Function(definition),
            StrongCallableDefinitionOwner::Function(definition),
            signature,
        )
        .map_err(ImportedMirCallableProjectionError::CallableShape)
    }
}

impl WireEncode for ImportedMirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportedMirCallableProjectionError {
    CallableShape(crate::ParamFreeMirCallableBuildError),
    FoundationNotCore(ConeIdentity),
    InitializationCycleRoleMismatch(PersistentFunctionId),
    MissingStrongSignature(PersistentFunctionId),
    StrongSignatureMismatch(PersistentFunctionId),
}

impl fmt::Display for ImportedMirCallableProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot project imported MIR callable: {self:?}")
    }
}

impl std::error::Error for ImportedMirCallableProjectionError {}

#[cfg(test)]
mod tests;
