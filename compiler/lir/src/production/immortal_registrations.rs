//! Complete semantic and writer-side production plans for strong immortal-object registrations.

use std::fmt;

pub use scoop_identity::PersistentImmortalObjectId;
use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentSymbolError, PersistentSymbolKey,
    PersistentSymbolRequest, StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::{ConeLirFoundation, DigestInputRefV1, RegistrationIdentitySurfaceV1};

mod semantics;
pub use semantics::*;

/// All identities and graph relations needed to emit one immortal-object registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongImmortalObjectRegistrationPlanV1 {
    semantic: StrongImmortalObjectSemanticPlanV1,
    definition_owner: crate::RegistrationDefinitionOwner,
    registration_symbol: PersistentSymbolRequest,
    registration_definition_plan: ObjectDefinitionPlanId,
    registration_primary_atom: ObjectDefinitionAtomId,
    object_definition_plan: ObjectDefinitionPlanId,
    object_definition_owner: scoop_identity::ObjectDefinitionPlanOwner,
    object_primary_atom: ObjectDefinitionAtomId,
    type_registration_symbol: PersistentSymbolRequest,
}

impl StrongImmortalObjectRegistrationPlanV1 {
    pub const fn definition_owner(self) -> crate::RegistrationDefinitionOwner {
        self.definition_owner
    }

    pub const fn semantic(self) -> StrongImmortalObjectSemanticPlanV1 {
        self.semantic
    }

    pub const fn object(self) -> PersistentImmortalObjectId {
        self.semantic.object()
    }

    pub const fn object_symbol(self) -> PersistentSymbolRequest {
        self.semantic.symbol()
    }

    pub const fn object_size(self) -> u64 {
        self.semantic.object_size()
    }

    pub const fn required_alignment(self) -> u64 {
        self.semantic.required_alignment()
    }

    pub const fn type_registration(self) -> PersistentExactTypeId {
        self.semantic.type_registration()
    }

    pub const fn registration_symbol(self) -> PersistentSymbolRequest {
        self.registration_symbol
    }

    pub const fn registration_definition_plan(self) -> ObjectDefinitionPlanId {
        self.registration_definition_plan
    }

    pub const fn registration_primary_atom(self) -> ObjectDefinitionAtomId {
        self.registration_primary_atom
    }

    pub const fn object_definition_plan(self) -> ObjectDefinitionPlanId {
        self.object_definition_plan
    }

    pub const fn object_definition_owner(self) -> scoop_identity::ObjectDefinitionPlanOwner {
        self.object_definition_owner
    }

    pub const fn object_primary_atom(self) -> ObjectDefinitionAtomId {
        self.object_primary_atom
    }

    pub const fn type_registration_symbol(self) -> PersistentSymbolRequest {
        self.type_registration_symbol
    }
}

/// Proof that every final LIR immortal object has one complete strong registration plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongImmortalObjectRegistrationPlanSetV1 {
    producer: ConeIdentity,
    registrations: Vec<StrongImmortalObjectRegistrationPlanV1>,
}

impl StrongImmortalObjectRegistrationPlanSetV1 {
    pub fn new(
        foundation: &ConeLirFoundation,
        identities: &RegistrationIdentitySurfaceV1,
        semantics: &StrongImmortalObjectSemanticPlanSetV1,
    ) -> Result<Self, StrongImmortalObjectRegistrationPlanBuildError> {
        if semantics.producer() != foundation.producer() {
            return Err(
                StrongImmortalObjectRegistrationPlanBuildError::ProducerMismatch {
                    foundation: foundation.producer(),
                    semantics: semantics.producer(),
                },
            );
        }
        let expected = semantics
            .objects()
            .iter()
            .map(|object| object.object())
            .collect::<Vec<_>>();
        let actual = identities
            .immortal_objects()
            .iter()
            .map(|registration| registration.semantic_id())
            .collect::<Vec<_>>();
        if expected != actual {
            return Err(StrongImmortalObjectRegistrationPlanBuildError::ObjectSet {
                expected,
                actual,
            });
        }
        for semantic in semantics.objects() {
            let local_type_registration_count = identities
                .type_registrations()
                .iter()
                .filter(|registration| registration.semantic_id() == semantic.type_registration())
                .count();
            if let ImmortalObjectTypeRegistrationRefV1::DependencyExternal { provider, .. } =
                semantic.type_registration_ref()
                && provider == foundation.producer()
            {
                return Err(StrongImmortalObjectRegistrationPlanBuildError::SelfImport {
                    object: semantic.object(),
                    provider,
                });
            }
            match semantic.type_registration_ref() {
                ImmortalObjectTypeRegistrationRefV1::Local(_)
                    if local_type_registration_count != 1 =>
                {
                    return Err(
                        StrongImmortalObjectRegistrationPlanBuildError::LocalTypeRegistrationSet {
                            object: semantic.object(),
                            type_registration: semantic.type_registration(),
                            actual: local_type_registration_count,
                        },
                    );
                }
                ImmortalObjectTypeRegistrationRefV1::DependencyExternal { .. }
                    if local_type_registration_count != 0 =>
                {
                    return Err(
                        StrongImmortalObjectRegistrationPlanBuildError::ExternalTypeRegistrationConflict {
                            object: semantic.object(),
                            type_registration: semantic.type_registration(),
                        },
                    );
                }
                ImmortalObjectTypeRegistrationRefV1::Local(_)
                | ImmortalObjectTypeRegistrationRefV1::DependencyExternal { .. } => {}
            }
        }
        let registrations = semantics
            .objects()
            .iter()
            .zip(identities.immortal_objects())
            .map(|(semantic, identity)| build_registration(foundation, semantic, identity))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            producer: foundation.producer(),
            registrations,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[StrongImmortalObjectRegistrationPlanV1] {
        &self.registrations
    }
}

fn build_registration(
    foundation: &ConeLirFoundation,
    semantic: &StrongImmortalObjectSemanticPlanV1,
    identity: &crate::RegistrationIdentityV1<PersistentImmortalObjectId>,
) -> Result<StrongImmortalObjectRegistrationPlanV1, StrongImmortalObjectRegistrationPlanBuildError>
{
    let object = semantic.object();
    let definition_owner = identity.owner();
    if !foundation.contains_immortal_object(object) {
        return Err(StrongImmortalObjectRegistrationPlanBuildError::MissingObject(object));
    }

    let object_definition = require_definition(
        foundation,
        StrongDefinitionEntity::immortal_object(object),
        StrongDefinitionRole::ImmortalObject,
    )?;
    let object_primary_atom = require_primary_atom(foundation, object_definition.id())?;
    require_symbol(foundation, semantic.symbol())?;

    let registration_definition = require_definition(
        foundation,
        StrongDefinitionEntity::immortal_object(object),
        StrongDefinitionRole::ImmortalRegistration,
    )?;
    if registration_definition.id() != identity.definition_plan() {
        return Err(
            StrongImmortalObjectRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                object,
                expected: registration_definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let registration_primary_atom = require_primary_atom(foundation, registration_definition.id())?;
    let registration_symbol = require_symbol_key(
        foundation,
        PersistentSymbolKey::ImmortalRegistration(object),
        definition_owner.linkage(),
    )?;
    let type_registration_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeRegistration(semantic.type_registration()),
        LinkageClass::ConeStrong,
    )
    .map_err(StrongImmortalObjectRegistrationPlanBuildError::Symbol)?;

    Ok(StrongImmortalObjectRegistrationPlanV1 {
        semantic: *semantic,
        definition_owner,
        registration_symbol,
        registration_definition_plan: registration_definition.id(),
        registration_primary_atom,
        object_definition_plan: object_definition.id(),
        object_definition_owner: object_definition.key().owner(),
        object_primary_atom,
        type_registration_symbol,
    })
}

fn require_definition(
    foundation: &ConeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<&crate::DefinitionPlanRecord, StrongImmortalObjectRegistrationPlanBuildError> {
    let key = ObjectDefinitionPlanKey::strong(foundation.producer(), entity, role)
        .map_err(StrongImmortalObjectRegistrationPlanBuildError::DefinitionIdentity)?;
    foundation
        .definition_for(entity, role)
        .ok_or(StrongImmortalObjectRegistrationPlanBuildError::MissingDefinition(Box::new(key)))
}

fn require_primary_atom(
    foundation: &ConeLirFoundation,
    definition: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, StrongImmortalObjectRegistrationPlanBuildError> {
    let atoms = foundation
        .definition_atoms_for_plan(definition)
        .filter(|record| record.key().role() == DefinitionAtomRole::Primary)
        .map(|record| record.id())
        .collect::<Vec<_>>();
    match atoms.as_slice() {
        [atom] => Ok(*atom),
        _ => Err(
            StrongImmortalObjectRegistrationPlanBuildError::PrimaryAtomSet {
                plan: definition,
                actual: atoms,
            },
        ),
    }
}

fn require_symbol_key(
    foundation: &ConeLirFoundation,
    key: PersistentSymbolKey,
    linkage: LinkageClass,
) -> Result<PersistentSymbolRequest, StrongImmortalObjectRegistrationPlanBuildError> {
    let symbol = PersistentSymbolRequest::new(key, linkage)
        .map_err(StrongImmortalObjectRegistrationPlanBuildError::Symbol)?;
    require_symbol(foundation, symbol)?;
    Ok(symbol)
}

fn require_symbol(
    foundation: &ConeLirFoundation,
    symbol: PersistentSymbolRequest,
) -> Result<(), StrongImmortalObjectRegistrationPlanBuildError> {
    if foundation.contains_symbol_request(symbol) {
        Ok(())
    } else {
        Err(StrongImmortalObjectRegistrationPlanBuildError::MissingSymbol(symbol))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongImmortalObjectRegistrationPlanBuildError {
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    ProducerMismatch {
        foundation: ConeIdentity,
        semantics: ConeIdentity,
    },
    ObjectSet {
        expected: Vec<PersistentImmortalObjectId>,
        actual: Vec<PersistentImmortalObjectId>,
    },
    MissingObject(PersistentImmortalObjectId),
    LocalTypeRegistrationSet {
        object: PersistentImmortalObjectId,
        type_registration: PersistentExactTypeId,
        actual: usize,
    },
    SelfImport {
        object: PersistentImmortalObjectId,
        provider: ConeIdentity,
    },
    ExternalTypeRegistrationConflict {
        object: PersistentImmortalObjectId,
        type_registration: PersistentExactTypeId,
    },
    MissingDefinition(Box<ObjectDefinitionPlanKey>),
    RegistrationDefinitionMismatch {
        object: PersistentImmortalObjectId,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingSymbol(PersistentSymbolRequest),
    MissingDigestNode(DigestNodeKey),
    RegistrationDigestMismatch {
        object: PersistentImmortalObjectId,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    DirectInputs {
        node: DigestNodeId,
        expected: Vec<DigestInputRefV1>,
        actual: Vec<DigestInputRefV1>,
    },
    RegistrationObjectInputs {
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    RegistrationObjectPatches {
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    ImmortalObjectInputs {
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    ImmortalObjectPatches {
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    PatchSet {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
        actual: Vec<DigestPatchIntentKey>,
    },
}

impl fmt::Display for StrongImmortalObjectRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong immortal-object registration plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongImmortalObjectRegistrationPlanBuildError {}

#[cfg(test)]
mod tests;
