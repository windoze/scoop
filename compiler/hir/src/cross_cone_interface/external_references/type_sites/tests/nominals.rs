use scoop_identity::{CoreBuiltinNominal, ExactTypeKey, NominalDeclarationOwner, NonEmptyVec};
use std::collections::BTreeMap;

use super::*;

#[test]
fn structural_exact_types_visit_only_reachable_nominal_leaves() {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let any = CoreBuiltinNominal::Any.identity_record().id();
    let mut keys = BTreeMap::new();
    let mut add = |key: ExactTypeKey| {
        let id = PersistentExactTypeId::from_key(&key).unwrap();
        keys.insert(id, key);
        id
    };
    let unit_exact = add(ExactTypeKey::Nominal(unit));
    let any_exact = add(ExactTypeKey::Nominal(any));
    let pointer = add(ExactTypeKey::RawPointer(unit_exact));
    let tuple = add(ExactTypeKey::Tuple(NonEmptyVec::from_first(
        any_exact,
        [pointer, unit_exact],
    )));
    let mut expected = vec![
        NominalDeclarationOwner::Concrete(unit),
        NominalDeclarationOwner::Concrete(any),
    ];
    expected.sort_unstable();
    let query = |root, meter: &mut BudgetMeter| {
        collect_type_site_nominals(root, |id| keys.get(&id).ok_or("missing exact"), meter)
    };
    assert_eq!(query(tuple, &mut meter()).unwrap(), expected);
    assert_eq!(
        query(pointer, &mut meter()).unwrap(),
        vec![NominalDeclarationOwner::Concrete(unit)]
    );
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
    ] {
        assert!(query(tuple, &mut BudgetMeter::new(limits)).is_err());
    }
}
