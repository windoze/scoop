//! Initialization pairs read from saved strong callable records.

use std::collections::BTreeMap;

use scoop_identity::CallableOwner;

use super::*;
use MirTypeBridgeUnitProblemV1 as Problem;
type Error = MirTypeBridgeSectionError;

pub fn read_initialization_units(
    strong: &crate::StrongCallableBridgeSurfaceV1,
    graph: &ValidatedIdentityGraph,
) -> Result<Vec<MirTypeBridgeInitializationUnitV1>, Error> {
    let mut pairs = BTreeMap::new();
    for definition in strong.bridges() {
        let CallableOwner::Generated(callable) = definition.implementation() else {
            continue;
        };
        let key = graph.canonical_key::<_, GeneratedCallableKey>(callable)?;
        let GeneratedCallableKey::Initialization { unit, role } = *key else {
            continue;
        };
        if definition.role() != crate::CallableRole::Ordinary {
            return Err(Error::Unit {
                unit,
                problem: Problem::DefinitionRole,
            });
        }
        let pair = pairs.entry(unit).or_insert((None, None));
        let entry = (callable, definition.signature());
        match role {
            InitializationCallableRole::Initializer => pair.0 = Some(entry),
            InitializationCallableRole::Ensure => pair.1 = Some(entry),
        }
    }
    let mut units = super::super::reserve(pairs.len())?;
    for (unit, (initializer, ensure)) in pairs {
        let (Some((initializer, signature)), Some((ensure, ensure_signature))) =
            (initializer, ensure)
        else {
            return Err(Error::Unit {
                unit,
                problem: Problem::MissingRole,
            });
        };
        if signature != ensure_signature {
            return Err(Error::Unit {
                unit,
                problem: Problem::Signature,
            });
        }
        units.push(MirTypeBridgeInitializationUnitV1::new(
            unit,
            initializer,
            ensure,
            MirBridgeCallableSignatureV1::new(signature.clone(), crate::GcEffect::Managed),
        ));
    }
    Ok(units)
}
