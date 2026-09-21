use std::fmt;

use scoop_identity::{
    CallableOwner, ConeIdentity, CoreImportedCallableKind, ExactCallableSignature,
    ImportedIdentityId, ImportedIdentityMap, MirIdentityLayer, PersistentFunctionId, PersistentId,
    StrongCallableDefinitionOwner,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalMirFoundation, MirFoundationCounts, OdrFreeMirFoundation, ValidatedMirFoundation,
};

mod selection;
pub use selection::*;

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
    pub fn project_initialization_cycle_thrower<'a>(
        &'a self,
        production: &'a crate::CoreBootstrapBridgeSectionV1,
        definition: PersistentFunctionId,
        signature: ExactCallableSignature,
    ) -> Result<SelectedImportedMirCallable<'a>, ImportedMirCallableProjectionError> {
        let kind = CoreImportedCallableKind::InitializationCycleThrower;
        if self.origin() != ConeIdentity::CORE {
            return Err(ImportedMirCallableProjectionError::FoundationNotCore(
                self.origin(),
            ));
        }
        let crate::CoreMirBridgeBranchV1::Core(core_bridge) = production.core_bridge() else {
            return Err(ImportedMirCallableProjectionError::MissingCoreBridge);
        };
        let bridge = core_bridge.initialization_cycle_thrower();
        let implementation = CallableOwner::Function(definition);
        if bridge.definition() != definition || bridge.implementation() != implementation {
            return Err(ImportedMirCallableProjectionError::CallableMismatch(kind));
        }
        self.project_checked_callable(production, kind, definition, implementation, signature)
    }

    fn project_checked_callable<'a>(
        &'a self,
        production: &'a crate::CoreBootstrapBridgeSectionV1,
        kind: CoreImportedCallableKind,
        definition: PersistentFunctionId,
        implementation: CallableOwner,
        signature: ExactCallableSignature,
    ) -> Result<SelectedImportedMirCallable<'a>, ImportedMirCallableProjectionError> {
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
        Ok(SelectedImportedMirCallable {
            foundation: self,
            production,
            kind,
            definition,
            implementation: StrongCallableDefinitionOwner::Function(definition),
            signature,
        })
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedMirCallableProjectionError {
    FoundationNotCore(ConeIdentity),
    MissingCoreBridge,
    CallableMismatch(CoreImportedCallableKind),
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
use selection::next_imported_core_mir_selection;

#[cfg(test)]
mod tests;
