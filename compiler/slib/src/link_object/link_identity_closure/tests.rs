use scoop_lir::GeneratedBridgePlanSetV1;
use scoop_wire::encode;

use super::*;
use crate::link_object::generated_c_objects::tests::{fixture, member_plan};

#[test]
fn materializations_are_sorted_by_member_and_keep_producer_kinds_distinct() {
    let fixture = fixture(Some("bridge"));
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
    let plan = member_plan(&fixture, &bridge_plan);
    let materializations = materializations(&plan);

    assert_eq!(materializations.len(), 2);
    assert!(materializations[0].member() < materializations[1].member());
    assert_ne!(
        encode(&materializations[0]).unwrap()[2],
        encode(&materializations[1]).unwrap()[2]
    );
}

#[test]
fn saved_uses_include_current_cone_and_external_callable_references() {
    use crate::link_object::strong_relocation_closure::tests::{
        verified_member_with_undefined, verified_member_without_relocations,
    };
    use crate::link_object::symbol_verification::tests::fixture_named;
    use crate::{
        PlannedStrongObjectSymbolRoleV1, verify_current_cone_strong_relocation_closure_v1,
    };

    let local = fixture_named("localTarget");
    let source = fixture_named("localCaller");
    let external = fixture_named("externalCaller");
    let second_source = fixture_named("secondLocalCaller");
    let second_external = fixture_named("secondExternalCaller");
    let target = local
        .symbols
        .symbols()
        .iter()
        .find(|symbol| {
            matches!(
                symbol.role(),
                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            )
        })
        .unwrap()
        .macho_name();
    let foreign = fixture_named("foreignTarget");
    let foreign_target = foreign
        .symbols
        .symbols()
        .iter()
        .find(|symbol| {
            matches!(
                symbol.role(),
                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            )
        })
        .unwrap()
        .macho_name();
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&local),
        verified_member_with_undefined(&source, target),
        verified_member_with_undefined(&external, foreign_target),
        verified_member_with_undefined(&second_source, target),
        verified_member_with_undefined(&second_external, foreign_target),
    ])
    .unwrap();
    let defined = CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure)
        .unwrap()
        .published();
    let uses = symbol_uses(&closure, &defined);
    assert_eq!(uses.len(), 2);
    assert!(
        uses.iter()
            .any(|usage| matches!(usage.target, uses::SymbolUseTarget::Defined(_)))
    );
    assert!(
        uses.iter()
            .any(|usage| matches!(usage.target, uses::SymbolUseTarget::External(_)))
    );
    for group in &uses {
        let bytes = encode(group).unwrap();
        let decoded: SymbolUseGroup = scoop_wire::decode_canonical(&bytes).unwrap();
        assert_eq!(&decoded, group);
        assert!(group.members.windows(2).all(|pair| pair[0] < pair[1]));
    }
    assert!(
        uses::SymbolUseTarget::Defined(u64::MAX)
            .symbol(&defined)
            .is_err()
    );
    for (member, symbol) in [
        (source.symbols.member(), target),
        (external.symbols.member(), foreign_target),
        (second_source.symbols.member(), target),
        (second_external.symbols.member(), foreign_target),
    ] {
        assert!(uses.iter().any(|usage| usage.members.contains(&member)
            && usage.target.symbol(&defined).unwrap() == symbol));
    }
}
