//! Complete writer-side production plans for strong safepoint registrations.

use std::fmt;

pub use scoop_identity::ObjectDefinitionAtomId;
use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentCallableBodyId,
    PersistentSafepointSiteId, PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
    SafepointId, SafepointSiteRole, StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::{
    DigestInputRefV1, DigestNodeV1, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongRegistrationIdentitySurfaceV1, StrongSafepointSemanticPlanSetV1,
};

/// All semantic identities and graph writers needed to emit one provisional
/// strong safepoint registration record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongSafepointRegistrationPlanV1 {
    site: PersistentSafepointSiteId,
    safepoint: SafepointId,
    owner: PersistentCallableBodyId,
    role: SafepointSiteRole,
    root_pair_count: u32,
    symbol: PersistentSymbolRequest,
    definition_plan: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    registration_fingerprint_node: DigestNodeId,
    normalized_stackmap_fingerprint_node: DigestNodeId,
    registration_definition_patch: DigestPatchIntentId,
    normalized_stackmap_patch: DigestPatchIntentId,
}

impl StrongSafepointRegistrationPlanV1 {
    pub const fn site(self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn safepoint(self) -> SafepointId {
        self.safepoint
    }

    pub const fn owner(self) -> PersistentCallableBodyId {
        self.owner
    }

    pub const fn role(self) -> SafepointSiteRole {
        self.role
    }

    pub const fn root_pair_count(self) -> u32 {
        self.root_pair_count
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

    pub const fn registration_fingerprint_node(self) -> DigestNodeId {
        self.registration_fingerprint_node
    }

    pub const fn normalized_stackmap_fingerprint_node(self) -> DigestNodeId {
        self.normalized_stackmap_fingerprint_node
    }

    pub const fn registration_definition_patch(self) -> DigestPatchIntentId {
        self.registration_definition_patch
    }

    pub const fn normalized_stackmap_patch(self) -> DigestPatchIntentId {
        self.normalized_stackmap_patch
    }
}

/// Proof that the current Cone has exactly one complete strong registration
/// production plan for every final LIR safepoint site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongSafepointRegistrationPlanSetV1 {
    producer: ConeIdentity,
    registrations: Vec<StrongSafepointRegistrationPlanV1>,
}

impl StrongSafepointRegistrationPlanSetV1 {
    pub fn new(
        foundation: &OdrFreeLirFoundation,
        identities: &StrongRegistrationIdentitySurfaceV1,
        semantics: &StrongSafepointSemanticPlanSetV1,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongSafepointRegistrationPlanBuildError> {
        if semantics.producer() != foundation.producer() {
            return Err(
                StrongSafepointRegistrationPlanBuildError::ProducerMismatch {
                    foundation: foundation.producer(),
                    semantics: semantics.producer(),
                },
            );
        }

        let expected = semantics
            .sites()
            .iter()
            .map(|site| site.site())
            .collect::<Vec<_>>();
        let actual = identities
            .safepoints()
            .iter()
            .map(|registration| registration.semantic_id())
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(StrongSafepointRegistrationPlanBuildError::SafepointSet {
                expected,
                actual,
            });
        }

        let registrations = semantics
            .sites()
            .iter()
            .zip(identities.safepoints())
            .map(|(semantic, identity)| build_registration(foundation, semantic, identity, digests))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            producer: foundation.producer(),
            registrations,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[StrongSafepointRegistrationPlanV1] {
        &self.registrations
    }
}

fn build_registration(
    foundation: &OdrFreeLirFoundation,
    semantic: &crate::StrongSafepointSemanticPlanV1,
    identity: &crate::StrongRegistrationIdentityV1<PersistentSafepointSiteId>,
    digests: &StrongDigestFinalizationPlanV1,
) -> Result<StrongSafepointRegistrationPlanV1, StrongSafepointRegistrationPlanBuildError> {
    let site = semantic.site();
    if !foundation.contains_safepoint_site(site) {
        return Err(StrongSafepointRegistrationPlanBuildError::MissingSite(site));
    }
    if !foundation.contains_safepoint_mapping(site, semantic.safepoint()) {
        return Err(
            StrongSafepointRegistrationPlanBuildError::MissingRuntimeMapping {
                site,
                safepoint: semantic.safepoint(),
            },
        );
    }
    if !foundation.contains_callable_body(semantic.owner()) {
        return Err(StrongSafepointRegistrationPlanBuildError::MissingOwner {
            site,
            owner: semantic.owner(),
        });
    }

    let definition_key = ObjectDefinitionPlanKey::strong(
        foundation.producer(),
        StrongDefinitionEntity::safepoint_site(site),
        StrongDefinitionRole::SafepointRegistration,
    )
    .map_err(StrongSafepointRegistrationPlanBuildError::DefinitionIdentity)?;
    let definition = foundation
        .definition_plans()
        .iter()
        .find(|record| record.key() == &definition_key)
        .ok_or(
            StrongSafepointRegistrationPlanBuildError::MissingDefinition(Box::new(definition_key)),
        )?;
    if identity.definition_plan() != definition.id() {
        return Err(
            StrongSafepointRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                site,
                expected: definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let primary_atoms = foundation
        .definition_atoms()
        .iter()
        .filter(|record| {
            record.key().plan() == definition.id()
                && record.key().role() == DefinitionAtomRole::Primary
        })
        .map(|record| record.id())
        .collect::<Vec<_>>();
    let primary_atom = match primary_atoms.as_slice() {
        [atom] => *atom,
        _ => {
            return Err(StrongSafepointRegistrationPlanBuildError::PrimaryAtomSet {
                plan: definition.id(),
                actual: primary_atoms,
            });
        }
    };

    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::SafepointRegistration(site),
        LinkageClass::ConeStrong,
    )
    .map_err(StrongSafepointRegistrationPlanBuildError::Symbol)?;
    if !foundation.contains_symbol_request(symbol) {
        return Err(StrongSafepointRegistrationPlanBuildError::MissingSymbol(
            symbol,
        ));
    }

    let object = require_digest_node(digests, DigestNodeKey::object_definition(primary_atom))?;
    if !object.direct_inputs().is_empty() {
        return Err(
            StrongSafepointRegistrationPlanBuildError::ObjectDefinitionInputs {
                node: object.id(),
                actual: object.direct_inputs().to_vec(),
            },
        );
    }
    if !object.patch_intents().is_empty() {
        return Err(
            StrongSafepointRegistrationPlanBuildError::ObjectDefinitionPatches {
                node: object.id(),
                actual: object
                    .patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            },
        );
    }
    let stackmap = require_digest_node(digests, DigestNodeKey::stackmap_record(site))?;
    let registration =
        require_digest_node(digests, DigestNodeKey::strong_registration(definition.id()))?;
    if registration.id() != identity.fingerprint_node() {
        return Err(
            StrongSafepointRegistrationPlanBuildError::RegistrationDigestMismatch {
                site,
                expected: registration.id(),
                actual: identity.fingerprint_node(),
            },
        );
    }

    let expected_inputs = vec![
        DigestInputRefV1::from_node(object),
        DigestInputRefV1::from_node(stackmap),
    ];
    if registration.direct_inputs() != expected_inputs {
        return Err(StrongSafepointRegistrationPlanBuildError::DirectInputs {
            node: registration.id(),
            expected: expected_inputs,
            actual: registration.direct_inputs().to_vec(),
        });
    }

    let target = definition_key.owner();
    let registration_definition_patch = require_only_patch(
        registration,
        DigestPatchIntentKey::new(
            registration.id(),
            target,
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        ),
    )?;
    let normalized_stackmap_patch = require_only_patch(
        stackmap,
        DigestPatchIntentKey::new(
            stackmap.id(),
            target,
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::NormalizedStackmap,
        ),
    )?;

    Ok(StrongSafepointRegistrationPlanV1 {
        site,
        safepoint: semantic.safepoint(),
        owner: semantic.owner(),
        role: semantic.role(),
        root_pair_count: semantic.root_pair_count(),
        symbol,
        definition_plan: definition.id(),
        primary_atom,
        registration_fingerprint_node: registration.id(),
        normalized_stackmap_fingerprint_node: stackmap.id(),
        registration_definition_patch,
        normalized_stackmap_patch,
    })
}

fn require_digest_node(
    digests: &StrongDigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongSafepointRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongSafepointRegistrationPlanBuildError::MissingDigestNode(key))
}

fn require_only_patch(
    node: &DigestNodeV1,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongSafepointRegistrationPlanBuildError> {
    match node.patch_intents() {
        [patch] if patch.key() == &expected => Ok(patch.id()),
        actual => Err(StrongSafepointRegistrationPlanBuildError::PatchSet {
            node: node.id(),
            expected: Box::new(expected),
            actual: actual.iter().map(|patch| *patch.key()).collect(),
        }),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongSafepointRegistrationPlanBuildError {
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    ProducerMismatch {
        foundation: ConeIdentity,
        semantics: ConeIdentity,
    },
    SafepointSet {
        expected: Vec<PersistentSafepointSiteId>,
        actual: Vec<PersistentSafepointSiteId>,
    },
    MissingSite(PersistentSafepointSiteId),
    MissingRuntimeMapping {
        site: PersistentSafepointSiteId,
        safepoint: SafepointId,
    },
    MissingOwner {
        site: PersistentSafepointSiteId,
        owner: PersistentCallableBodyId,
    },
    MissingDefinition(Box<ObjectDefinitionPlanKey>),
    RegistrationDefinitionMismatch {
        site: PersistentSafepointSiteId,
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
        site: PersistentSafepointSiteId,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    DirectInputs {
        node: DigestNodeId,
        expected: Vec<DigestInputRefV1>,
        actual: Vec<DigestInputRefV1>,
    },
    ObjectDefinitionInputs {
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    ObjectDefinitionPatches {
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    PatchSet {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
        actual: Vec<DigestPatchIntentKey>,
    },
}

impl fmt::Display for StrongSafepointRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong safepoint registration plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongSafepointRegistrationPlanBuildError {}

#[cfg(test)]
mod tests;
