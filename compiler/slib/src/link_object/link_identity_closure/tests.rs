use scoop_lir::GeneratedBridgePlanSetV1;
use scoop_wire::encode;

use super::*;
use crate::link_object::c_bridge_production::tests::{fixture, member_plan};

#[test]
fn materializations_are_sorted_by_member_and_keep_producer_kinds_distinct() {
    let fixture = fixture(Some("bridge"));
    let bridge_plan =
        GeneratedBridgePlanSetV1::from_odr_free_foundation(&fixture.foundation).unwrap();
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
fn entry_owner_library_is_the_fixed_empty_sum() {
    assert_eq!(
        encode(&VerifiedEntryOwnerBranchV1::Library).unwrap(),
        vec![0xa1, 0x00, 0x01]
    );
}
