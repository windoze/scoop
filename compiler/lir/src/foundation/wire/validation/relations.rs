use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    CallbackApplicationKey, CallbackRegistrationKey, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitKey, IdentityLayer, ObjectDefinitionPlanOwner,
    PersistentCallbackApplicationId, PersistentCallbackRegistrationId,
    PersistentSourceNativeExternalContractId, SafepointSiteRole, SourceNativeExternalContractKey,
    ValidatedIdentityGraph,
};

use super::*;

pub(super) fn validate_safepoints(
    callable_bodies: &[CallableBodyRecord],
    sites: &[SafepointSiteRecord],
    mappings: &[SafepointMappingRecord],
) -> Result<(), SafepointRelationError> {
    let bodies = callable_bodies
        .iter()
        .map(RuntimeIdentityRecord::id)
        .collect::<BTreeSet<_>>();
    let mut ordinals =
        BTreeMap::<(PersistentCallableBodyId, SafepointSiteRole), BTreeSet<u32>>::new();
    let mut site_ids = BTreeSet::new();
    for site in sites {
        let key = site.key();
        if !bodies.contains(&key.owner()) {
            return Err(SafepointRelationError::MissingOwner {
                site: site.id(),
                owner: key.owner(),
            });
        }
        site_ids.insert(site.id());
        let values = ordinals.entry((key.owner(), key.role())).or_default();
        if !values.insert(key.ordinal()) {
            return Err(SafepointRelationError::DuplicateOrdinal {
                owner: key.owner(),
                role: key.role(),
                ordinal: key.ordinal(),
            });
        }
    }
    for ((owner, role), values) in ordinals {
        for (expected, actual) in (0_u32..).zip(values) {
            if expected != actual {
                return Err(SafepointRelationError::NonContiguousOrdinal {
                    owner,
                    role,
                    expected,
                    actual,
                });
            }
        }
    }

    let mut mapped = BTreeSet::new();
    for mapping in mappings {
        if !site_ids.contains(&mapping.site()) {
            return Err(SafepointRelationError::UnexpectedMapping {
                site: mapping.site(),
            });
        }
        if !mapped.insert(mapping.site()) {
            return Err(SafepointRelationError::DuplicateMapping {
                site: mapping.site(),
            });
        }
    }
    if let Some(site) = site_ids.into_iter().find(|site| !mapped.contains(site)) {
        return Err(SafepointRelationError::MissingMapping { site });
    }
    Ok(())
}

pub(super) fn validate_native_contracts(
    identities: &ValidatedIdentityGraph,
    contracts: &[NativeExternalContractRecord],
) -> Result<(), LirFoundationValidationError> {
    let sources = identities
        .records::<PersistentSourceNativeExternalContractId, SourceNativeExternalContractKey>(
            IdentityLayer::Hir,
        )
        .map_err(LirFoundationValidationError::Identity)?
        .into_iter()
        .map(|record| record.id())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for contract in contracts {
        if !sources.contains(&contract.source()) {
            return Err(NativeContractRelationError::UnexpectedRecord {
                source: contract.source(),
            }
            .into());
        }
        if !seen.insert(contract.source()) {
            return Err(NativeContractRelationError::DuplicateRecord {
                source: contract.source(),
            }
            .into());
        }
    }
    if let Some(source) = sources.into_iter().find(|source| !seen.contains(source)) {
        return Err(NativeContractRelationError::MissingRecord { source }.into());
    }
    Ok(())
}

pub(super) struct BridgeTables<'a> {
    pub contracts: &'a [NativeExternalContractRecord],
    pub signatures: &'a [CanonicalCAbiSignatureFingerprintRecord],
    pub layouts: &'a [CanonicalCAbiLayoutFingerprintRecord],
    pub units: &'a [BridgeUnitRecord],
    pub atoms: &'a [BridgeAtomRecord],
    pub callbacks: &'a [CallbackBridgeRecord],
    pub plans: &'a [DefinitionPlanRecord],
}

pub(super) fn validate_bridges(
    identities: &ValidatedIdentityGraph,
    producer: ConeIdentity,
    tables: BridgeTables<'_>,
) -> Result<(), LirFoundationValidationError> {
    let applications = identities
        .records::<PersistentCallbackApplicationId, CallbackApplicationKey>(IdentityLayer::Mir)
        .map_err(LirFoundationValidationError::Identity)?
        .into_iter()
        .map(|record| (record.id(), record.into_key()))
        .collect::<BTreeMap<_, _>>();
    let registrations = identities
        .records::<PersistentCallbackRegistrationId, CallbackRegistrationKey>(IdentityLayer::Hir)
        .map_err(LirFoundationValidationError::Identity)?
        .into_iter()
        .map(|record| (record.id(), record.into_key()))
        .collect::<BTreeMap<_, _>>();
    let contract_fingerprints = tables
        .contracts
        .iter()
        .map(NativeExternalContractRecord::fingerprint)
        .collect::<BTreeSet<_>>();
    let signature_fingerprints = tables
        .signatures
        .iter()
        .map(CanonicalCAbiSignatureFingerprintRecord::fingerprint)
        .collect::<BTreeSet<_>>();
    let layout_fingerprints = tables
        .layouts
        .iter()
        .map(CanonicalCAbiLayoutFingerprintRecord::fingerprint)
        .collect::<BTreeSet<_>>();
    let units = tables
        .units
        .iter()
        .map(|record| (record.id(), *record.key()))
        .collect::<BTreeMap<_, _>>();

    for (&unit, key) in &units {
        match *key {
            GeneratedBridgeUnitKey::OutboundFunction(contract)
            | GeneratedBridgeUnitKey::GlobalRead(contract)
            | GeneratedBridgeUnitKey::GlobalWrite(contract)
            | GeneratedBridgeUnitKey::GlobalAddress(contract) => {
                if !contract_fingerprints.contains(&contract) {
                    return Err(
                        BridgeRelationError::MissingNativeContract { unit, contract }.into(),
                    );
                }
            }
            GeneratedBridgeUnitKey::CallbackTrampoline { signature, .. }
            | GeneratedBridgeUnitKey::StaticCallbackTrampoline { signature, .. } => {
                if !signature_fingerprints.contains(&signature) {
                    return Err(
                        BridgeRelationError::MissingUnitSignature { unit, signature }.into(),
                    );
                }
            }
        }
    }

    let mut seen_applications = BTreeSet::new();
    let mut referenced_callback_units = BTreeSet::new();
    for callback in tables.callbacks {
        let application = callback.application();
        if !seen_applications.insert(application) {
            return Err(BridgeRelationError::DuplicateCallbackRecord { application }.into());
        }
        let application_key = applications
            .get(&application)
            .ok_or(BridgeRelationError::UnexpectedCallbackRecord { application })?;
        let registration = registrations.get(&application_key.registration()).ok_or(
            BridgeRelationError::MissingCallbackRegistration {
                application,
                registration: application_key.registration(),
            },
        )?;
        if !signature_fingerprints.contains(&callback.signature()) {
            return Err(BridgeRelationError::MissingCallbackSignature {
                application,
                signature: callback.signature(),
            }
            .into());
        }
        let unit_key =
            units
                .get(&callback.unit())
                .ok_or(BridgeRelationError::MissingCallbackUnit {
                    application,
                    unit: callback.unit(),
                })?;
        let expected = GeneratedBridgeUnitKey::CallbackTrampoline {
            signature: callback.signature(),
            context_index: registration.context_index(),
        };
        if *unit_key != expected {
            return Err(BridgeRelationError::CallbackUnitMismatch {
                application,
                unit: callback.unit(),
            }
            .into());
        }
        referenced_callback_units.insert(callback.unit());
    }
    if let Some(application) = applications
        .keys()
        .find(|application| !seen_applications.contains(application))
    {
        return Err(BridgeRelationError::MissingCallbackRecord {
            application: *application,
        }
        .into());
    }
    if let Some(unit) = units.iter().find_map(|(&unit, key)| {
        matches!(key, GeneratedBridgeUnitKey::CallbackTrampoline { .. })
            .then_some(unit)
            .filter(|unit| !referenced_callback_units.contains(unit))
    }) {
        return Err(BridgeRelationError::UnusedCallbackUnit { unit }.into());
    }

    for atom in tables.atoms {
        let key = atom.key();
        if key.producer() != producer {
            return Err(LirFoundationOwnershipError::ForeignBridgeAtom {
                atom: atom.id(),
                expected: producer,
                actual: key.producer(),
            }
            .into());
        }
        let role = key.atom();
        let unit = role.unit();
        let unit_key = units
            .get(&unit)
            .ok_or(BridgeRelationError::MissingAtomUnit {
                atom: atom.id(),
                unit,
            })?;
        match role {
            GeneratedBridgeAtomRoleKey::PrimaryEntry { .. } => {}
            GeneratedBridgeAtomRoleKey::SignatureDescriptor { signature, .. } => {
                if !signature_fingerprints.contains(&signature) {
                    return Err(BridgeRelationError::MissingAtomSignature {
                        atom: atom.id(),
                        signature,
                    }
                    .into());
                }
                if let GeneratedBridgeUnitKey::CallbackTrampoline {
                    signature: expected,
                    ..
                } = unit_key
                    && signature != *expected
                {
                    return Err(BridgeRelationError::AtomSignatureMismatch {
                        atom: atom.id(),
                        unit,
                    }
                    .into());
                }
            }
            GeneratedBridgeAtomRoleKey::ContextDescriptor { context_index, .. } => {
                let GeneratedBridgeUnitKey::CallbackTrampoline {
                    context_index: expected,
                    ..
                } = unit_key
                else {
                    return Err(BridgeRelationError::ContextForNonCallbackUnit {
                        atom: atom.id(),
                        unit,
                    }
                    .into());
                };
                if context_index != *expected {
                    return Err(BridgeRelationError::AtomContextMismatch {
                        atom: atom.id(),
                        unit,
                    }
                    .into());
                }
            }
            GeneratedBridgeAtomRoleKey::StaticAssertSupport { layout, .. } => {
                if !layout_fingerprints.contains(&layout) {
                    return Err(BridgeRelationError::MissingAtomLayout {
                        atom: atom.id(),
                        layout,
                    }
                    .into());
                }
            }
        }
    }

    for plan in tables.plans {
        if let ObjectDefinitionPlanOwner::Strong {
            producer: actual, ..
        } = plan.key().owner()
            && actual != producer
        {
            return Err(LirFoundationOwnershipError::ForeignStrongDefinitionPlan {
                plan: plan.id(),
                expected: producer,
                actual,
            }
            .into());
        }
    }
    Ok(())
}
