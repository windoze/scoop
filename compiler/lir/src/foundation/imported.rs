use std::fmt;
use std::rc::Rc;

use scoop_identity::{
    ConeIdentity, CoreImportedCallableKind, ExactCallableSignature, ImportedIdentityId,
    ImportedIdentityMap, LirIdentityLayer, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentId, StrongCallableDefinitionOwner,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalLirFoundation, LirFoundationCounts, OdrFreeLirFoundation, ValidatedLirFoundation,
};

/// Session-local LIR identity. It cannot be interchanged with imported HIR or
/// MIR identities even when the persistent kind is the same.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedLirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedLirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// The LIR identity foundation after atomic import into a semantic session.
pub struct ImportedLirFoundation {
    canonical: Rc<CanonicalLirFoundation>,
    identities: ImportedIdentityMap<LirIdentityLayer>,
}

impl ImportedLirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedLirFoundation,
        identities: ImportedIdentityMap<LirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: Rc::new(foundation.into_canonical()),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeLirFoundation,
        identities: ImportedIdentityMap<LirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_shared(),
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> LirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedLirId<I>> {
        self.identities.get(id).map(ImportedLirId)
    }

    /// Projects the compiler service through its typed bridge and common strong definition.
    pub fn project_initialization_cycle_thrower(
        &self,
        bridge: &crate::CallableAbiRecordV1,
        definitions: &crate::StrongObjectSymbolSurfaceV1,
        target: StrongCallableDefinitionOwner,
        signature: ExactCallableSignature,
    ) -> Result<crate::SelectedDependencyLirCallableV1, ImportedLirCallableProjectionError> {
        let kind = CoreImportedCallableKind::InitializationCycleThrower;
        if bridge.target() != target {
            return Err(ImportedLirCallableProjectionError::TargetMismatch(kind));
        }
        if bridge.abi_signature().signature() != &signature {
            return Err(ImportedLirCallableProjectionError::SignatureMismatch(kind));
        }
        let (body, _, required_definition) = bridge
            .link_contract(self.origin())
            .map_err(ImportedLirCallableProjectionError::Callable)?;
        self.identity(body)
            .ok_or(ImportedLirCallableProjectionError::MissingBody(body))?;
        self.identity(required_definition).ok_or(
            ImportedLirCallableProjectionError::MissingDefinition(required_definition),
        )?;
        bridge
            .validate_definition(self.origin(), definitions)
            .map_err(ImportedLirCallableProjectionError::Callable)?;
        let StrongCallableDefinitionOwner::Function(function) = target else {
            return Err(ImportedLirCallableProjectionError::TargetMismatch(kind));
        };
        crate::SelectedDependencyLirCallableV1::new(
            self.origin(),
            scoop_identity::DependencyCallableDeclarationId::Function(function),
            target,
            bridge.abi_signature().clone(),
            bridge.calling_convention(),
            bridge.root_plan(),
        )
        .map_err(ImportedLirCallableProjectionError::Record)
    }
}

impl WireEncode for ImportedLirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

#[derive(Debug)]
pub enum ImportedLirCallableProjectionError {
    Record(crate::ParamFreeLirCallableBuildError),
    TargetMismatch(CoreImportedCallableKind),
    SignatureMismatch(CoreImportedCallableKind),
    Callable(crate::CallableAbiValidationError),
    MissingBody(PersistentCallableBodyId),
    MissingDefinition(ObjectDefinitionPlanId),
}

impl fmt::Display for ImportedLirCallableProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot project imported LIR callable: {self:?}")
    }
}

impl std::error::Error for ImportedLirCallableProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Callable(error) => Some(error),
            Self::Record(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
