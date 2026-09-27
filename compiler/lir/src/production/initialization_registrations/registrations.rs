//! Closed writer-side production plans for strong initialization registrations.

use std::fmt;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey,
    DigestPatchIntentId, DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentCallableBodyId,
    PersistentInitializationUnitId, PersistentStaticStorageId, PersistentSymbolError,
    PersistentSymbolKey, PersistentSymbolRequest, StrongDefinitionEntity, StrongDefinitionRole,
};

use super::{
    StrongInitializationSchedulePlanV1, StrongInitializationUnitSemanticPlan,
    StrongInitializationUnitSemanticPlanSet,
};
use crate::{
    ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1, DigestNodeV1,
    RegistrationIdentitySurfaceV1,
};

mod build;
mod external_edges;
mod model;
mod validation;
pub use model::*;

impl<D: crate::StrongInitializationDependencyReference>
    StrongInitializationUnitRegistrationPlanSet<D>
{
    pub fn new(
        foundation: &ConeLirFoundation,
        identities: &RegistrationIdentitySurfaceV1,
        semantics: &StrongInitializationUnitSemanticPlanSet<D>,
        digests: &DigestFinalizationPlanV1,
    ) -> Result<Self, StrongInitializationUnitRegistrationPlanBuildError> {
        if semantics.producer() != foundation.producer() {
            return Err(
                StrongInitializationUnitRegistrationPlanBuildError::ProducerMismatch {
                    foundation: foundation.producer(),
                    semantics: semantics.producer(),
                },
            );
        }
        let expected = semantics
            .units()
            .iter()
            .map(StrongInitializationUnitSemanticPlan::unit)
            .collect::<Vec<_>>();
        let actual = identities
            .initialization_units()
            .iter()
            .map(|identity| identity.semantic_id())
            .collect::<Vec<_>>();
        if expected != actual {
            return Err(
                StrongInitializationUnitRegistrationPlanBuildError::UnitSet { expected, actual },
            );
        }

        let registrations = semantics
            .units()
            .iter()
            .zip(identities.initialization_units())
            .map(|(semantic, identity)| {
                build::build_registration(foundation, identities, semantic, identity, digests)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            producer: foundation.producer(),
            registrations,
        })
    }
}

impl<D> StrongInitializationUnitRegistrationPlanSet<D> {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[StrongInitializationUnitRegistrationPlan<D>] {
        &self.registrations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationObjectLeafV1 {
    Registration,
    Cell,
    Descriptor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationReferencedRegistrationV1 {
    StaticStorage(PersistentStaticStorageId),
    Callable(PersistentCallableBodyId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationUnitRegistrationPlanBuildError {
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    ProducerMismatch {
        foundation: ConeIdentity,
        semantics: ConeIdentity,
    },
    UnitSet {
        expected: Vec<PersistentInitializationUnitId>,
        actual: Vec<PersistentInitializationUnitId>,
    },
    SelfDependency(PersistentInitializationUnitId),
    DependencyOrder {
        unit: PersistentInitializationUnitId,
        index: usize,
    },
    DependencyProviderRole {
        unit: PersistentInitializationUnitId,
        dependency: PersistentInitializationUnitId,
    },
    MissingStaticStorage(PersistentStaticStorageId),
    MissingStaticStorageRegistration(PersistentStaticStorageId),
    MissingCallableBody(PersistentCallableBodyId),
    MissingCallableRegistration(PersistentCallableBodyId),
    MissingDefinition(Box<ObjectDefinitionPlanKey>),
    RegistrationDefinitionMismatch {
        unit: PersistentInitializationUnitId,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    ReferencedRegistrationDefinitionMismatch {
        registration: InitializationReferencedRegistrationV1,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    AssociatedAtomSet {
        unit: PersistentInitializationUnitId,
        definition: ObjectDefinitionPlanId,
        expected: Vec<ObjectDefinitionAtomKey>,
        actual: Vec<ObjectDefinitionAtomKey>,
    },
    MissingSymbol(PersistentSymbolRequest),
    MissingDigestNode(DigestNodeKey),
    RegistrationDigestMismatch {
        unit: PersistentInitializationUnitId,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    ReferencedRegistrationDigestMismatch {
        registration: InitializationReferencedRegistrationV1,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    DirectInputs {
        node: DigestNodeId,
        expected: Vec<DigestInputRefV1>,
        actual: Vec<DigestInputRefV1>,
    },
    ObjectLeafInputs {
        leaf: InitializationObjectLeafV1,
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    ObjectLeafPatches {
        leaf: InitializationObjectLeafV1,
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    PatchSet {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
        actual: Vec<DigestPatchIntentKey>,
    },
    GatewayPatchSet {
        registration: ObjectDefinitionPlanId,
        expected: Option<Box<DigestPatchIntentKey>>,
        actual: Vec<DigestPatchIntentKey>,
    },
}

impl fmt::Display for StrongInitializationUnitRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong initialization-unit registration plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongInitializationUnitRegistrationPlanBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefinitionIdentity(source) => Some(source),
            Self::Symbol(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
pub(in crate::production) mod tests;
