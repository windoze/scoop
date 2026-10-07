use std::collections::{BTreeSet, HashMap, HashSet};

use scoop_identity::{
    CallbackApplicationKey, CallbackRegistrationKey, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitKey, IdentityLayer, ObjectDefinitionPlanOwner,
    PersistentCallbackApplicationId, PersistentCallbackRegistrationId,
    PersistentSourceNativeExternalContractId, SourceNativeExternalContractKey,
    ValidatedIdentityGraph,
};
use scoop_wire::WirePath;

use super::*;

pub(super) fn validate_safepoints(
    callable_bodies: &[CallableBodyRecord],
    sites: &[SafepointSiteRecord],
    mappings: &[SafepointMappingRecord],
) -> Result<(), LirFoundationValidationError> {
    let path = WirePath::root().field(10);
    let mut bodies = HashSet::new();
    scoop_wire::allocation::try_reserve_set(&mut bodies, callable_bodies.len(), &path)
        .map_err(LirFoundationValidationError::Resource)?;
    bodies.extend(callable_bodies.iter().map(RuntimeIdentityRecord::id));

    let mut ordinal_values = HashSet::new();
    scoop_wire::allocation::try_reserve_set(&mut ordinal_values, sites.len(), &path)
        .map_err(LirFoundationValidationError::Resource)?;
    let mut site_ids = HashSet::new();
    scoop_wire::allocation::try_reserve_set(&mut site_ids, sites.len(), &path)
        .map_err(LirFoundationValidationError::Resource)?;
    for site in sites {
        let key = site.key();
        if !bodies.contains(&key.owner()) {
            return Err(SafepointRelationError::MissingOwner {
                site: site.id(),
                owner: key.owner(),
            }
            .into());
        }
        site_ids.insert(site.id());
        let group = (key.owner(), key.role());
        if !ordinal_values.insert((group, key.ordinal())) {
            return Err(SafepointRelationError::DuplicateOrdinal {
                owner: key.owner(),
                role: key.role(),
                ordinal: key.ordinal(),
            }
            .into());
        }
    }

    let mapping_path = WirePath::root().field(12);
    let mut mapped = HashSet::new();
    scoop_wire::allocation::try_reserve_set(&mut mapped, mappings.len(), &mapping_path)
        .map_err(LirFoundationValidationError::Resource)?;
    for mapping in mappings {
        if !site_ids.contains(&mapping.site()) {
            return Err(SafepointRelationError::UnexpectedMapping {
                site: mapping.site(),
            }
            .into());
        }
        if !mapped.insert(mapping.site()) {
            return Err(SafepointRelationError::DuplicateMapping {
                site: mapping.site(),
            }
            .into());
        }
    }
    if let Some(site) = sites
        .iter()
        .map(CborIdentityRecord::id)
        .find(|site| !mapped.contains(site))
    {
        return Err(SafepointRelationError::MissingMapping { site }.into());
    }
    Ok(())
}

pub(super) fn validate_native_contracts(
    identities: &ValidatedIdentityGraph,
    contracts: &[NativeExternalContractRecord],
) -> Result<(), LirFoundationValidationError> {
    let source_records = identities
        .records::<PersistentSourceNativeExternalContractId, SourceNativeExternalContractKey>(
            IdentityLayer::Hir,
            &WirePath::root().field(26),
        )
        .map_err(LirFoundationValidationError::Identity)?;
    let path = WirePath::root().field(14);
    let mut sources = HashSet::new();
    scoop_wire::allocation::try_reserve_set(&mut sources, source_records.len(), &path)
        .map_err(LirFoundationValidationError::Resource)?;
    sources.extend(source_records.iter().map(CborIdentityRecord::id));
    let mut seen = HashSet::new();
    scoop_wire::allocation::try_reserve_set(&mut seen, contracts.len(), &path)
        .map_err(LirFoundationValidationError::Resource)?;
    for contract in contracts {
        if !sources.contains(&contract.source())
            && identities
                .canonical_key::<_, SourceNativeExternalContractKey>(contract.source())
                .is_err()
        {
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
    if let Some(source) = source_records
        .iter()
        .map(CborIdentityRecord::id)
        .find(|source| !seen.contains(source))
    {
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
    let application_records = identities
        .records::<PersistentCallbackApplicationId, CallbackApplicationKey>(
            IdentityLayer::Mir,
            &WirePath::root().field(9),
        )
        .map_err(LirFoundationValidationError::Identity)?;
    let callback_path = WirePath::root().field(19);
    let mut applications = HashMap::new();
    scoop_wire::allocation::try_reserve_map(
        &mut applications,
        application_records.len(),
        &callback_path,
    )
    .map_err(LirFoundationValidationError::Resource)?;
    applications.extend(
        application_records
            .iter()
            .map(|record| (record.id(), record.key())),
    );

    let registration_records = identities
        .records::<PersistentCallbackRegistrationId, CallbackRegistrationKey>(
            IdentityLayer::Hir,
            &WirePath::root().field(25),
        )
        .map_err(LirFoundationValidationError::Identity)?;
    let mut registrations = HashMap::new();
    scoop_wire::allocation::try_reserve_map(
        &mut registrations,
        registration_records.len(),
        &callback_path,
    )
    .map_err(LirFoundationValidationError::Resource)?;
    registrations.extend(
        registration_records
            .iter()
            .map(|record| (record.id(), record.key())),
    );

    let contract_path = WirePath::root().field(14);
    let mut contract_fingerprints = HashSet::new();
    scoop_wire::allocation::try_reserve_set(
        &mut contract_fingerprints,
        tables.contracts.len(),
        &contract_path,
    )
    .map_err(LirFoundationValidationError::Resource)?;
    contract_fingerprints.extend(
        tables
            .contracts
            .iter()
            .map(NativeExternalContractRecord::fingerprint),
    );

    let signature_path = WirePath::root().field(15);
    let mut signature_fingerprints = HashSet::new();
    scoop_wire::allocation::try_reserve_set(
        &mut signature_fingerprints,
        tables.signatures.len(),
        &signature_path,
    )
    .map_err(LirFoundationValidationError::Resource)?;
    signature_fingerprints.extend(
        tables
            .signatures
            .iter()
            .map(CanonicalCAbiSignatureFingerprintRecord::fingerprint),
    );

    let layout_path = WirePath::root().field(16);
    let mut layout_fingerprints = HashSet::new();
    scoop_wire::allocation::try_reserve_set(
        &mut layout_fingerprints,
        tables.layouts.len(),
        &layout_path,
    )
    .map_err(LirFoundationValidationError::Resource)?;
    layout_fingerprints.extend(
        tables
            .layouts
            .iter()
            .map(CanonicalCAbiLayoutFingerprintRecord::fingerprint),
    );

    let unit_path = WirePath::root().field(17);
    let mut units = HashMap::new();
    scoop_wire::allocation::try_reserve_map(&mut units, tables.units.len(), &unit_path)
        .map_err(LirFoundationValidationError::Resource)?;
    units.extend(
        tables
            .units
            .iter()
            .map(|record| (record.id(), *record.key())),
    );

    for record in tables.units {
        let unit = record.id();
        match *record.key() {
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

    let mut seen_applications = HashSet::new();
    scoop_wire::allocation::try_reserve_set(
        &mut seen_applications,
        tables.callbacks.len(),
        &callback_path,
    )
    .map_err(LirFoundationValidationError::Resource)?;
    let mut referenced_callback_units = HashSet::new();
    scoop_wire::allocation::try_reserve_set(
        &mut referenced_callback_units,
        tables.callbacks.len(),
        &callback_path,
    )
    .map_err(LirFoundationValidationError::Resource)?;
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
    if let Some(application) = application_records
        .iter()
        .map(CborIdentityRecord::id)
        .find(|application| !seen_applications.contains(application))
    {
        return Err(BridgeRelationError::MissingCallbackRecord { application }.into());
    }
    if let Some(unit) = tables.units.iter().find_map(|record| {
        matches!(
            record.key(),
            GeneratedBridgeUnitKey::CallbackTrampoline { .. }
        )
        .then_some(record.id())
        .filter(|unit| !referenced_callback_units.contains(unit))
    }) {
        return Err(BridgeRelationError::UnusedCallbackUnit { unit }.into());
    }

    let mut actual_static_asserts = std::collections::BTreeMap::<_, BTreeSet<_>>::new();
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
                } = *unit_key
                    && signature != expected
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
                } = *unit_key
                else {
                    return Err(BridgeRelationError::ContextForNonCallbackUnit {
                        atom: atom.id(),
                        unit,
                    }
                    .into());
                };
                if context_index != expected {
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
                actual_static_asserts
                    .entry(unit)
                    .or_default()
                    .insert(layout);
            }
        }
    }
    let required_static_asserts = required_generated_bridge_layouts(
        bridge_unit_keys(tables.units),
        tables.contracts,
        tables.signatures,
        tables.layouts,
    )
    .map_err(BridgeRelationError::StaticAssertLayoutClosure)?;
    for (&unit, expected) in &required_static_asserts {
        let actual = actual_static_asserts.get(&unit);
        if let Some(&layout) = expected
            .iter()
            .find(|layout| actual.is_none_or(|actual| !actual.contains(layout)))
        {
            return Err(BridgeRelationError::MissingStaticAssertLayout { unit, layout }.into());
        }
    }
    for (unit, actual) in actual_static_asserts {
        let expected = required_static_asserts.get(&unit);
        if let Some(layout) = actual
            .into_iter()
            .find(|layout| expected.is_none_or(|expected| !expected.contains(layout)))
        {
            return Err(BridgeRelationError::UnexpectedStaticAssertLayout { unit, layout }.into());
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
