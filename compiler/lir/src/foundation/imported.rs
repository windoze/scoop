use std::fmt;

use scoop_identity::{
    ConeIdentity, CoreImportedCallableKind, ExactCallableSignature, ImportedIdentityId,
    ImportedIdentityMap, LirIdentityLayer, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentId, StrongCallableDefinitionOwner,
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
    canonical: CanonicalLirFoundation,
    identities: ImportedIdentityMap<LirIdentityLayer>,
}

impl ImportedLirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedLirFoundation,
        identities: ImportedIdentityMap<LirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeLirFoundation,
        identities: ImportedIdentityMap<LirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
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

    /// Projects one provider-owned TypeDescriptor only after its exact type and
    /// canonical strong definition have both been proven by this imported
    /// LIR world. Public-surface capability checks remain the responsibility
    /// of the artifact-level caller that supplies `target`.
    pub fn project_type_descriptor(
        &self,
        definitions: &crate::StrongObjectSymbolSurfaceV1,
        target: PersistentExactTypeId,
    ) -> Result<crate::ExternalTypeDescriptor, ImportedLirTypeDescriptorProjectionError> {
        if self
            .canonical
            .materialized_exact_types
            .binary_search(&target)
            .is_err()
        {
            return Err(ImportedLirTypeDescriptorProjectionError::MissingExactType(
                target,
            ));
        }
        let descriptor = crate::ExternalTypeDescriptor::new(self.origin(), target)
            .map_err(ImportedLirTypeDescriptorProjectionError::Contract)?;
        let required_definition = descriptor.required_definition();
        self.identity(required_definition).ok_or(
            ImportedLirTypeDescriptorProjectionError::MissingDefinition(required_definition),
        )?;
        descriptor
            .validate_definition(definitions)
            .map_err(ImportedLirTypeDescriptorProjectionError::Definition)?;
        Ok(descriptor)
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

#[derive(Debug)]
pub enum ImportedLirTypeDescriptorProjectionError {
    MissingExactType(PersistentExactTypeId),
    Contract(crate::ExternalTypeDescriptorBuildError),
    MissingDefinition(ObjectDefinitionPlanId),
    Definition(crate::ExternalTypeDescriptorValidationError),
}

impl fmt::Display for ImportedLirTypeDescriptorProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot project imported LIR TypeDescriptor: {self:?}"
        )
    }
}

impl std::error::Error for ImportedLirTypeDescriptorProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            Self::Definition(error) => Some(error),
            _ => None,
        }
    }
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
