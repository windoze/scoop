//! Complete writer-side production plans for strong type registrations.

use std::fmt;

pub use scoop_identity::PersistentExactTypeId;
use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey,
    DigestPatchIntentId, DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, ObjectDefinitionPlanOwner,
    ObjectDefinitionPlanRole, PersistentLayoutId, PersistentSymbolError, PersistentSymbolKey,
    PersistentSymbolRequest, RepresentationRole, RuntimeTypeId, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole, TargetProfileWireId,
};

use crate::{
    ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1, DigestNodeV1,
    RegistrationIdentitySurfaceV1, TypeDescriptorInlineScanV1,
};

mod semantics;
pub use semantics::*;

mod model;
pub use model::*;
mod build;
use build::build_registration;
mod validation;
use validation::*;

impl<D: StrongDescriptorReference, C: Clone> StrongTypeRegistrationPlanSet<D, C> {
    pub fn new(
        target: crate::LirTargetProfile,
        foundation: &ConeLirFoundation,
        identities: &RegistrationIdentitySurfaceV1,
        semantics: &StrongTypeDescriptorSemanticPlanSet<D, C>,
        digests: &DigestFinalizationPlanV1,
    ) -> Result<Self, StrongTypeRegistrationPlanBuildError> {
        if semantics.producer() != foundation.producer() {
            return Err(StrongTypeRegistrationPlanBuildError::ProducerMismatch {
                foundation: foundation.producer(),
                semantics: semantics.producer(),
            });
        }
        if semantics.target() != &target.wire_id() {
            return Err(StrongTypeRegistrationPlanBuildError::TargetMismatch {
                expected: target.wire_id(),
                actual: semantics.target().clone(),
            });
        }
        let mut expected = foundation
            .definition_plans()
            .iter()
            .filter_map(
                |record| match (record.key().owner(), record.key().definition_role()) {
                    (
                        ObjectDefinitionPlanOwner::Strong { entity, .. },
                        ObjectDefinitionPlanRole::Strong(StrongDefinitionRole::TypeDescriptor),
                    ) => match entity.kind() {
                        StrongDefinitionEntityKind::ExactType(exact) => Some(exact),
                        _ => None,
                    },
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        expected.sort_unstable();
        if let Some(pair) = expected.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(StrongTypeRegistrationPlanBuildError::DuplicateDescriptor(
                pair[0],
            ));
        }
        let semantic_types = semantics
            .descriptors()
            .iter()
            .map(StrongTypeDescriptorSemanticPlan::exact_type)
            .collect::<Vec<_>>();
        if expected != semantic_types {
            return Err(
                StrongTypeRegistrationPlanBuildError::DescriptorSemanticSet {
                    expected,
                    actual: semantic_types,
                },
            );
        }
        let actual = identities
            .type_registrations()
            .iter()
            .map(|registration| registration.semantic_id())
            .collect::<Vec<_>>();
        if semantic_types != actual {
            return Err(StrongTypeRegistrationPlanBuildError::TypeSet {
                expected: semantic_types,
                actual,
            });
        }

        let registrations = semantics
            .descriptors()
            .iter()
            .zip(identities.type_registrations())
            .map(|(semantic, identity)| {
                build_registration(target, foundation, semantic, identity, digests)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            producer: foundation.producer(),
            target: target.wire_id(),
            registrations,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn target(&self) -> &TargetProfileWireId {
        &self.target
    }

    pub fn registrations(&self) -> &[StrongTypeRegistrationPlan<D, C>] {
        &self.registrations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeRegistrationPlanBuildError {
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    DuplicateDescriptor(PersistentExactTypeId),
    ProducerMismatch {
        foundation: ConeIdentity,
        semantics: ConeIdentity,
    },
    TargetMismatch {
        expected: TargetProfileWireId,
        actual: TargetProfileWireId,
    },
    DescriptorSemanticSet {
        expected: Vec<PersistentExactTypeId>,
        actual: Vec<PersistentExactTypeId>,
    },
    TypeSet {
        expected: Vec<PersistentExactTypeId>,
        actual: Vec<PersistentExactTypeId>,
    },
    MissingDefinition(Box<ObjectDefinitionPlanKey>),
    RegistrationDefinitionMismatch {
        exact_type: PersistentExactTypeId,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    DescriptorAssociatedAtomSet {
        exact_type: PersistentExactTypeId,
        expected: Vec<ObjectDefinitionAtomKey>,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingSymbol(PersistentSymbolRequest),
    RuntimeTypeSet {
        exact_type: PersistentExactTypeId,
        actual: Vec<RuntimeTypeId>,
    },
    MissingLayout(PersistentLayoutId),
    MissingScan(scoop_identity::PersistentScanId),
    InstanceLayoutMismatch {
        exact_type: PersistentExactTypeId,
        layout: PersistentLayoutId,
    },
    InstanceScanMismatch {
        exact_type: PersistentExactTypeId,
        scan: scoop_identity::PersistentScanId,
    },
    VtableMismatch {
        exact_type: PersistentExactTypeId,
        table: scoop_identity::PersistentDispatchTableId,
    },
    ItableMismatch {
        exact_type: PersistentExactTypeId,
        table: scoop_identity::PersistentDispatchTableId,
    },
    MissingDigestNode(DigestNodeKey),
    RegistrationObjectInputs {
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    RegistrationObjectPatches {
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    RegistrationDigestMismatch {
        exact_type: PersistentExactTypeId,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    DirectInputs {
        node: DigestNodeId,
        expected: Vec<DigestInputRefV1>,
        actual: Vec<DigestInputRefV1>,
    },
    PatchSet {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
        actual: Vec<DigestPatchIntentKey>,
    },
    MissingPatch {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
    },
}

impl fmt::Display for StrongTypeRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong type registration plan: {self:?}")
    }
}

impl std::error::Error for StrongTypeRegistrationPlanBuildError {}

#[cfg(test)]
mod tests;
