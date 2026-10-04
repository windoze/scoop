//! Complete writer-side production plans for strong callable registrations.

use std::fmt;

pub use scoop_identity::PersistentCallableBodyId;
use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolError, PersistentSymbolKey,
    PersistentSymbolRequest, StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::{
    ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1, DigestNodeV1,
    RegistrationIdentitySurfaceV1,
};

mod runtime_scans;
pub use runtime_scans::*;

/// All semantic identities and graph writers required to emit one strong
/// callable registration record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongCallableRegistrationPlanV1 {
    body: PersistentCallableBodyId,
    context_key_count: u64,
    symbol: PersistentSymbolRequest,
    definition_plan: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    entry_symbol: PersistentSymbolRequest,
    body_definition_plan: ObjectDefinitionPlanId,
    body_primary_atom: ObjectDefinitionAtomId,
    registration_object_node: DigestNodeId,
    body_definition_node: DigestNodeId,
    registration_fingerprint_node: DigestNodeId,
    definition_owner: crate::RegistrationDefinitionOwner,
    registration_definition_patch: DigestPatchIntentId,
    body_definition_patch: DigestPatchIntentId,
}

impl StrongCallableRegistrationPlanV1 {
    pub const fn context_key_count(self) -> u64 {
        self.context_key_count
    }
    pub const fn definition_owner(self) -> crate::RegistrationDefinitionOwner {
        self.definition_owner
    }

    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn symbol(self) -> PersistentSymbolRequest {
        self.symbol
    }

    pub const fn definition_plan(self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn primary_atom(self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn entry_symbol(self) -> PersistentSymbolRequest {
        self.entry_symbol
    }

    pub const fn body_definition_plan(self) -> ObjectDefinitionPlanId {
        self.body_definition_plan
    }

    pub const fn body_primary_atom(self) -> ObjectDefinitionAtomId {
        self.body_primary_atom
    }

    pub const fn registration_object_node(self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn body_definition_node(self) -> DigestNodeId {
        self.body_definition_node
    }

    pub const fn registration_fingerprint_node(self) -> DigestNodeId {
        self.registration_fingerprint_node
    }

    pub const fn registration_definition_patch(self) -> DigestPatchIntentId {
        self.registration_definition_patch
    }

    pub const fn body_definition_patch(self) -> DigestPatchIntentId {
        self.body_definition_patch
    }
}

/// Proof that every callable body has exactly one complete strong callable
/// registration production plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongCallableRegistrationPlanSetV1 {
    producer: ConeIdentity,
    registrations: Vec<StrongCallableRegistrationPlanV1>,
    runtime_scans: StrongCallableRuntimeScanPlanSetV1,
}

impl StrongCallableRegistrationPlanSetV1 {
    pub fn new(
        foundation: &ConeLirFoundation,
        identities: &RegistrationIdentitySurfaceV1,
        runtime_scans: StrongCallableRuntimeScanPlanSetV1,
        digests: &DigestFinalizationPlanV1,
    ) -> Result<Self, StrongCallableRegistrationPlanBuildError> {
        let mut expected = foundation
            .callable_bodies()
            .iter()
            .map(|body| body.id())
            .collect::<Vec<_>>();
        expected.sort_unstable();
        let actual = identities
            .callables()
            .iter()
            .map(|registration| registration.semantic_id())
            .collect::<Vec<_>>();
        if expected != actual {
            return Err(StrongCallableRegistrationPlanBuildError::CallableSet { expected, actual });
        }
        let actual_runtime_scan_bodies = runtime_scans
            .callables()
            .iter()
            .map(StrongCallableRuntimeScanPlanV1::body)
            .collect::<Vec<_>>();
        if runtime_scans.producer() != foundation.producer()
            || actual_runtime_scan_bodies != expected
        {
            return Err(
                StrongCallableRegistrationPlanBuildError::RuntimeScanCallableSet {
                    expected,
                    actual: actual_runtime_scan_bodies,
                },
            );
        }

        for callable in runtime_scans.callables() {
            let definition = require_definition(
                foundation,
                StrongDefinitionEntity::callable_body(callable.body()),
                StrongDefinitionRole::CallableBody,
            )?;
            let actual = foundation
                .definition_atoms()
                .iter()
                .filter(|atom| {
                    atom.key().plan() == definition.id()
                        && atom.key().role() == DefinitionAtomRole::RuntimeRecord
                })
                .map(|atom| atom.id())
                .collect::<Vec<_>>();
            let mut expected_atoms = callable
                .atoms()
                .iter()
                .map(StrongCallableRuntimeScanAtomV1::atom)
                .collect::<Vec<_>>();
            expected_atoms.sort_unstable();
            if actual != expected_atoms {
                return Err(
                    StrongCallableRegistrationPlanBuildError::RuntimeScanAtomSet {
                        body: callable.body(),
                        expected: expected_atoms,
                        actual,
                    },
                );
            }
        }

        let mut registrations = Vec::with_capacity(expected.len());
        for (body, identity) in expected.into_iter().zip(identities.callables()) {
            let context_key_count = runtime_scans
                .callable(body)
                .expect("the callable support set has already been matched")
                .context_keys()
                .len() as u64;
            let registration = build_registration(
                foundation,
                digests,
                body,
                identity.definition_plan(),
                identity.fingerprint_node(),
                identity.owner(),
                context_key_count,
            )?;
            registrations.push(registration);
        }
        Ok(Self {
            producer: foundation.producer(),
            registrations,
            runtime_scans,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[StrongCallableRegistrationPlanV1] {
        &self.registrations
    }

    pub const fn runtime_scans(&self) -> &StrongCallableRuntimeScanPlanSetV1 {
        &self.runtime_scans
    }
}

fn build_registration(
    foundation: &ConeLirFoundation,
    digests: &DigestFinalizationPlanV1,
    body: PersistentCallableBodyId,
    identity_definition: ObjectDefinitionPlanId,
    identity_fingerprint: DigestNodeId,
    definition_owner: crate::RegistrationDefinitionOwner,
    context_key_count: u64,
) -> Result<StrongCallableRegistrationPlanV1, StrongCallableRegistrationPlanBuildError> {
    let registration_definition = require_definition(
        foundation,
        StrongDefinitionEntity::callable_body(body),
        StrongDefinitionRole::CallableRegistration,
    )?;
    if registration_definition.id() != identity_definition {
        return Err(
            StrongCallableRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                body,
                expected: registration_definition.id(),
                actual: identity_definition,
            },
        );
    }
    let primary_atom = require_primary_atom(foundation, registration_definition.id())?;
    let symbol = require_symbol(
        foundation,
        PersistentSymbolKey::CallableRegistration(body),
        definition_owner.linkage(),
    )?;

    let body_definition = require_definition(
        foundation,
        StrongDefinitionEntity::callable_body(body),
        StrongDefinitionRole::CallableBody,
    )?;
    let body_primary_atom = require_primary_atom(foundation, body_definition.id())?;
    let entry_symbol = require_symbol(
        foundation,
        PersistentSymbolKey::CallableBody(body),
        definition_owner.linkage(),
    )?;

    let registration_object =
        require_digest_node(digests, DigestNodeKey::object_definition(primary_atom))?;
    let expected_object_inputs = match definition_owner {
        crate::RegistrationDefinitionOwner::Strong => Vec::new(),
        crate::RegistrationDefinitionOwner::Odr { .. } => vec![DigestInputRefV1::ObjectDefinition(
            DigestNodeId::from_key(&DigestNodeKey::object_definition(body_primary_atom))
                .map_err(StrongCallableRegistrationPlanBuildError::Hash)?,
        )],
    };
    if registration_object.direct_inputs() != expected_object_inputs {
        return Err(
            StrongCallableRegistrationPlanBuildError::RegistrationObjectInputs {
                node: registration_object.id(),
                actual: registration_object.direct_inputs().to_vec(),
            },
        );
    }
    if !registration_object.patch_intents().is_empty() {
        return Err(
            StrongCallableRegistrationPlanBuildError::RegistrationObjectPatches {
                node: registration_object.id(),
                actual: registration_object
                    .patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            },
        );
    }
    let body_definition_node =
        require_digest_node(digests, DigestNodeKey::object_definition(body_primary_atom))?;
    let registration_fingerprint = require_digest_node(
        digests,
        definition_owner.digest_key(registration_definition.id()),
    )?;
    if registration_fingerprint.id() != identity_fingerprint {
        return Err(
            StrongCallableRegistrationPlanBuildError::RegistrationDigestMismatch {
                body,
                expected: registration_fingerprint.id(),
                actual: identity_fingerprint,
            },
        );
    }

    let mut expected_inputs = vec![DigestInputRefV1::from_node(registration_object)];
    expected_inputs.push(match definition_owner {
        crate::RegistrationDefinitionOwner::Strong => {
            DigestInputRefV1::from_node(body_definition_node)
        }
        crate::RegistrationDefinitionOwner::Odr { .. } => DigestInputRefV1::from_node(
            require_digest_node(digests, DigestNodeKey::lir_definition(primary_atom))?,
        ),
    });
    expected_inputs.sort_unstable();
    if registration_fingerprint.direct_inputs() != expected_inputs {
        return Err(StrongCallableRegistrationPlanBuildError::DirectInputs {
            node: registration_fingerprint.id(),
            expected: expected_inputs,
            actual: registration_fingerprint.direct_inputs().to_vec(),
        });
    }

    let registration_definition_patch = require_only_patch(
        registration_fingerprint,
        DigestPatchIntentKey::new(
            registration_fingerprint.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        ),
    )?;
    let body_definition_patch = require_patch(
        body_definition_node,
        DigestPatchIntentKey::new(
            body_definition_node.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::CallableBodyDefinition,
        ),
    )?;

    Ok(StrongCallableRegistrationPlanV1 {
        body,
        context_key_count,
        symbol,
        definition_plan: registration_definition.id(),
        primary_atom,
        entry_symbol,
        body_definition_plan: body_definition.id(),
        body_primary_atom,
        registration_object_node: registration_object.id(),
        body_definition_node: body_definition_node.id(),
        registration_fingerprint_node: registration_fingerprint.id(),
        definition_owner,
        registration_definition_patch,
        body_definition_patch,
    })
}

fn require_definition(
    foundation: &ConeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<
    &scoop_identity::CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    StrongCallableRegistrationPlanBuildError,
> {
    foundation
        .definition_for(entity, role)
        .ok_or(StrongCallableRegistrationPlanBuildError::MissingDefinition { entity, role })
}

fn require_primary_atom(
    foundation: &ConeLirFoundation,
    plan: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, StrongCallableRegistrationPlanBuildError> {
    let actual = foundation
        .definition_atoms()
        .iter()
        .filter(|atom| {
            atom.key().plan() == plan && atom.key().role() == DefinitionAtomRole::Primary
        })
        .map(|atom| atom.id())
        .collect::<Vec<_>>();
    match actual.as_slice() {
        [atom] => Ok(*atom),
        _ => Err(StrongCallableRegistrationPlanBuildError::PrimaryAtomSet { plan, actual }),
    }
}

fn require_symbol(
    foundation: &ConeLirFoundation,
    key: PersistentSymbolKey,
    linkage: LinkageClass,
) -> Result<PersistentSymbolRequest, StrongCallableRegistrationPlanBuildError> {
    let request = PersistentSymbolRequest::new(key, linkage)
        .map_err(StrongCallableRegistrationPlanBuildError::Symbol)?;
    foundation
        .contains_symbol_request(request)
        .then_some(request)
        .ok_or(StrongCallableRegistrationPlanBuildError::MissingSymbol(
            request,
        ))
}

fn require_digest_node(
    digests: &DigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongCallableRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongCallableRegistrationPlanBuildError::MissingDigestNode(
            key,
        ))
}

fn require_only_patch(
    node: &DigestNodeV1,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongCallableRegistrationPlanBuildError> {
    match node.patch_intents() {
        [patch] if patch.key() == &expected => Ok(patch.id()),
        actual => Err(StrongCallableRegistrationPlanBuildError::PatchSet {
            node: node.id(),
            expected: Box::new(expected),
            actual: actual.iter().map(|patch| *patch.key()).collect(),
        }),
    }
}

fn require_patch(
    node: &DigestNodeV1,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongCallableRegistrationPlanBuildError> {
    node.patch_intents()
        .iter()
        .find(|patch| patch.key() == &expected)
        .map(|patch| patch.id())
        .ok_or(StrongCallableRegistrationPlanBuildError::MissingPatch {
            node: node.id(),
            expected: Box::new(expected),
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableRegistrationPlanBuildError {
    Hash(scoop_wire::HashError),
    Symbol(PersistentSymbolError),
    CallableSet {
        expected: Vec<PersistentCallableBodyId>,
        actual: Vec<PersistentCallableBodyId>,
    },
    RuntimeScanCallableSet {
        expected: Vec<PersistentCallableBodyId>,
        actual: Vec<PersistentCallableBodyId>,
    },
    RuntimeScanAtomSet {
        body: PersistentCallableBodyId,
        expected: Vec<ObjectDefinitionAtomId>,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingDefinition {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    RegistrationDefinitionMismatch {
        body: PersistentCallableBodyId,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingSymbol(PersistentSymbolRequest),
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
        body: PersistentCallableBodyId,
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

impl fmt::Display for StrongCallableRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong callable registration plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongCallableRegistrationPlanBuildError {}

#[cfg(test)]
mod tests;
