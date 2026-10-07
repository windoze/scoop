//! Complete writer-side production plans for strong safepoint registrations.

use std::fmt;

pub use scoop_identity::ObjectDefinitionAtomId;
use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId,
    DigestPatchIntentKey, DigestSemanticFieldRole, ObjectDefinitionPlanId,
    PersistentCallableBodyId, PersistentSafepointSiteId, PersistentSymbolError,
    PersistentSymbolKey, PersistentSymbolRequest, SafepointId, SafepointSiteRole,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::{
    ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1, DigestNodeV1,
    RegistrationIdentitySurfaceV1, StrongSafepointSemanticPlanSetV1,
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
    definition_owner: crate::RegistrationDefinitionOwner,
    normalized_stackmap_fingerprint_node: DigestNodeId,
    normalized_stackmap_patch: DigestPatchIntentId,
}

impl StrongSafepointRegistrationPlanV1 {
    pub const fn definition_owner(self) -> crate::RegistrationDefinitionOwner {
        self.definition_owner
    }

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

    pub const fn normalized_stackmap_fingerprint_node(self) -> DigestNodeId {
        self.normalized_stackmap_fingerprint_node
    }

    pub const fn normalized_stackmap_patch(self) -> DigestPatchIntentId {
        self.normalized_stackmap_patch
    }
}

/// Complete registration plans for the current Cone's retained safepoints.
/// Codegen finalization projects logical LIR leaves into physical root counts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongSafepointRegistrationPlanSetV1 {
    producer: ConeIdentity,
    registrations: Vec<StrongSafepointRegistrationPlanV1>,
}

impl StrongSafepointRegistrationPlanSetV1 {
    pub(crate) fn set_emitted_root_counts(
        &mut self,
        counts: &std::collections::BTreeMap<SafepointId, u32>,
    ) -> Result<(), StrongSafepointRegistrationPlanBuildError> {
        let known = self
            .registrations
            .iter()
            .map(|plan| plan.safepoint)
            .collect::<std::collections::BTreeSet<_>>();
        if counts.keys().any(|site| !known.contains(site)) {
            return Err(StrongSafepointRegistrationPlanBuildError::EmittedSiteSet);
        }
        self.registrations
            .retain(|plan| counts.contains_key(&plan.safepoint));
        for plan in &mut self.registrations {
            plan.root_pair_count = counts[&plan.safepoint];
        }
        Ok(())
    }

    pub fn new(
        foundation: &ConeLirFoundation,
        identities: &RegistrationIdentitySurfaceV1,
        semantics: &StrongSafepointSemanticPlanSetV1,
        digests: &DigestFinalizationPlanV1,
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
    foundation: &ConeLirFoundation,
    semantic: &crate::StrongSafepointSemanticPlanV1,
    identity: &crate::RegistrationIdentityV1<PersistentSafepointSiteId>,
    digests: &DigestFinalizationPlanV1,
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

    let definition_owner = identity.owner();
    let definition = foundation
        .definition_for(
            StrongDefinitionEntity::safepoint_site(site),
            StrongDefinitionRole::SafepointRegistration,
        )
        .ok_or(StrongSafepointRegistrationPlanBuildError::MissingDefinition(site))?;
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
        definition_owner.linkage(),
    )
    .map_err(StrongSafepointRegistrationPlanBuildError::Symbol)?;
    if !foundation.contains_symbol_request(symbol) {
        return Err(StrongSafepointRegistrationPlanBuildError::MissingSymbol(
            symbol,
        ));
    }

    let stackmap = require_digest_node(digests, DigestNodeKey::stackmap_record(site))?;
    let normalized_stackmap_patch = require_only_patch(
        stackmap,
        DigestPatchIntentKey::new(
            stackmap.id(),
            definition.id(),
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
        definition_owner,
        normalized_stackmap_fingerprint_node: stackmap.id(),
        normalized_stackmap_patch,
    })
}

fn require_digest_node(
    digests: &DigestFinalizationPlanV1,
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
    EmittedSiteSet,
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
    MissingDefinition(PersistentSafepointSiteId),
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
