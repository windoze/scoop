//! Initialization contracts replayed from original source keys and MIR definitions.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    CallableOwner, CborIdentityRecord, CoreBuiltinNominal, Effect, ExactCallableSignature,
    ExactTypeKey, InitializationUnitKey, PersistentExactTypeId,
};

use super::*;
use MirTypeBridgeUnitProblemV1 as Problem;
type Error = MirTypeBridgeSectionError;

/// Source-only records do not imply a machine root. Every actual initialization
/// role must resolve to an original source unit and its complete strong pair.
pub fn replay_source_initialization_units(
    provider: ConeIdentity,
    source_units: &[CborIdentityRecord<PersistentInitializationUnitId, InitializationUnitKey>],
    foundation: &crate::CanonicalMirFoundation,
    strong: &crate::StrongCallableBridgeSurfaceV1,
    graph: &ValidatedIdentityGraph,
    unit_result: PersistentExactTypeId,
) -> Result<Vec<MirTypeBridgeInitializationUnitV1>, Error> {
    let mut sources = BTreeMap::new();
    for record in source_units {
        if sources.insert(record.id(), record.key()).is_some() {
            return Err(Error::NonCanonicalUnitInventory);
        }
    }
    let mut required = BTreeSet::new();
    for definition in strong.bridges() {
        let CallableOwner::Generated(callable) = definition.implementation() else {
            continue;
        };

        let key = graph.canonical_key::<_, GeneratedCallableKey>(callable)?;
        let GeneratedCallableKey::Initialization { unit, .. } = *key else {
            continue;
        };

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

        if !required.contains(&unit) {
            required.insert(unit);
        }
    }
    let mut units = super::super::reserve(required.len())?;
    for unit in required {
        // Provider resolution follows both the unit key and its source owner.

        if super::super::super::objects::unit_provider(graph, unit)? != provider {
            return Err(Error::Unit {
                unit,
                problem: Problem::WrongProvider,
            });
        }

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
        )?;
        let ensure = role(
            unit,
            InitializationCallableRole::Ensure,
            &exact,
            foundation,
            strong,
            graph,
        )?;
        units.push(MirTypeBridgeInitializationUnitV1 {
            unit,
            initializer,
            ensure,
            signature: MirBridgeCallableSignatureV1::new(exact, crate::GcEffect::Managed),
        });
    }
    Ok(units)
}

fn role(
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
    expected: &ExactCallableSignature,
    foundation: &crate::CanonicalMirFoundation,
    strong: &crate::StrongCallableBridgeSurfaceV1,
    graph: &ValidatedIdentityGraph,
) -> Result<StrongCallableDefinitionOwner, Error> {
    let key = GeneratedCallableKey::Initialization { unit, role };

    let callable = PersistentGeneratedCallableId::from_key(&key)?;

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
    let signatures = foundation.callable_signatures();

    let actual = signatures
        .binary_search_by(|record| record.subject().compare_sort_key(definition.subject()))
        .ok()
        .map(|index| signatures[index].signature());

    if definition.signature() != expected || actual != Some(expected) {
        return Err(Error::Unit {
            unit,
            problem: Problem::Signature,
        });
    }
    Ok(StrongCallableDefinitionOwner::GeneratedCallable(callable))
}
