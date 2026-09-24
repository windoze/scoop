//! Initialization contracts replayed from original source keys and MIR definitions.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
};

use scoop_identity::{
    CallableOwner, CborIdentityRecord, CoreBuiltinNominal, Effect, ExactCallableSignature,
    ExactTypeKey, InitializationUnitKey, PersistentExactTypeId,
};

use super::*;
use MirTypeBridgeUnitProblemV1 as Problem;
type Error = MirTypeBridgeSectionError<Infallible>;

/// Source-only records do not imply a machine root. Every actual initialization
/// role must resolve to an original source unit and its complete strong pair.
pub fn replay_source_initialization_units(
    provider: ConeIdentity,
    source_units: &[CborIdentityRecord<PersistentInitializationUnitId, InitializationUnitKey>],
    foundation: &crate::OdrFreeMirFoundation,
    strong: &crate::StrongCallableBridgeSurfaceV1,
    graph: &ValidatedIdentityGraph,
    unit_result: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<Vec<MirTypeBridgeInitializationUnitV1>, Error> {
    let path = WirePath::root();
    let mut sources = BTreeMap::new();
    for record in source_units {
        lookup(sources.len(), meter)?;
        meter.check_table_entries(sources.len() as u64 + 1, &path)?;
        meter.charge_collection_slots(1, &path)?;
        if sources.insert(record.id(), record.key()).is_some() {
            return Err(Error::NonCanonicalUnitInventory);
        }
    }
    let mut required = BTreeSet::new();
    for definition in strong.bridges() {
        meter.charge_work(1, &path)?;
        let CallableOwner::Generated(callable) = definition.implementation() else {
            continue;
        };
        lookup(graph.identity_count(), meter)?;
        let key = graph.canonical_key::<_, GeneratedCallableKey>(callable)?;
        let GeneratedCallableKey::Initialization { unit, .. } = *key else {
            continue;
        };
        lookup(sources.len(), meter)?;
        let source = sources.get(&unit).ok_or(Error::Unit {
            unit,
            problem: Problem::MissingSource,
        })?;
        if matches!(
            source,
            InitializationUnitKey::GenericDelegatedExtensionApplication { .. }
        ) {
            return Err(Error::Unit {
                unit,
                problem: Problem::GenericSource,
            });
        }
        lookup(graph.identity_count(), meter)?;
        if graph
            .canonical_key::<_, InitializationUnitKey>(unit)?
            .as_ref()
            != *source
        {
            return Err(Error::Unit {
                unit,
                problem: Problem::MissingSource,
            });
        }
        lookup(required.len(), meter)?;
        if !required.contains(&unit) {
            meter.check_table_entries(required.len() as u64 + 1, &path)?;
            meter.charge_collection_slots(1, &path)?;
            required.insert(unit);
        }
    }
    let mut units = super::super::reserve(required.len(), meter)?;
    for unit in required {
        // Provider resolution follows both the unit key and its source owner.
        lookup(graph.identity_count(), meter)?;
        lookup(graph.identity_count(), meter)?;
        if super::super::super::objects::unit_provider(graph, unit)? != provider {
            return Err(Error::Unit {
                unit,
                problem: Problem::WrongProvider,
            });
        }
        lookup(graph.identity_count(), meter)?;
        if graph
            .canonical_key::<_, ExactTypeKey>(unit_result)?
            .as_ref()
            != &ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
        {
            return Err(Error::Unit {
                unit,
                problem: Problem::Signature,
            });
        }
        let exact = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit_result);
        let initializer = role(
            unit,
            InitializationCallableRole::Initializer,
            &exact,
            foundation,
            strong,
            graph,
            meter,
        )?;
        let ensure = role(
            unit,
            InitializationCallableRole::Ensure,
            &exact,
            foundation,
            strong,
            graph,
            meter,
        )?;
        units.push(MirTypeBridgeInitializationUnitV1 {
            unit,
            initializer,
            ensure,
            signature: MirBridgeCallableSignatureV1::new(exact, crate::GcEffect::Managed),
            proof: MirInitializationUnitProofKindV1::ReaderSemanticReplay,
        });
    }
    Ok(units)
}

fn role(
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
    expected: &ExactCallableSignature,
    foundation: &crate::OdrFreeMirFoundation,
    strong: &crate::StrongCallableBridgeSurfaceV1,
    graph: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<StrongCallableDefinitionOwner, Error> {
    let key = GeneratedCallableKey::Initialization { unit, role };
    meter.charge_sha256(
        PersistentGeneratedCallableId::hash_stream_length(&key)?,
        &WirePath::root(),
    )?;
    let callable = PersistentGeneratedCallableId::from_key(&key)?;
    lookup(graph.identity_count(), meter)?;
    if graph
        .canonical_key::<_, GeneratedCallableKey>(callable)?
        .as_ref()
        != &key
    {
        return Err(Error::Unit {
            unit,
            problem: Problem::MissingRole,
        });
    }
    let target = CallableOwner::Generated(callable);
    lookup(strong.bridges().len(), meter)?;
    let definition = strong.get(target).ok_or(Error::Unit {
        unit,
        problem: Problem::MissingRole,
    })?;
    if definition.role() != crate::CallableRole::Ordinary {
        return Err(Error::Unit {
            unit,
            problem: Problem::DefinitionRole,
        });
    }
    let signatures = foundation.as_canonical().callable_signatures();
    lookup(signatures.len(), meter)?;
    let actual = signatures
        .binary_search_by(|record| record.subject().compare_sort_key(definition.subject()))
        .ok()
        .map(|index| signatures[index].signature());
    meter.charge_work(
        definition.signature().parameters().len() as u64 + 5,
        &WirePath::root(),
    )?;
    if definition.signature() != expected || actual != Some(expected) {
        return Err(Error::Unit {
            unit,
            problem: Problem::Signature,
        });
    }
    Ok(StrongCallableDefinitionOwner::GeneratedCallable(callable))
}

fn lookup(count: usize, meter: &mut BudgetMeter) -> Result<(), WireError> {
    meter.charge_work(u64::from(count.max(1).ilog2()) + 1, &WirePath::root())
}
