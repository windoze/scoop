use super::*;
use crate::link::map::ConeMapSymbol;
use scoop_slib::{MemberStableKey, SlibMemberRecord, SlibMemberRole};

fn object(cone: ConeIdentity) -> ObjectKey {
    let member = SlibMemberRecord::new(
        cone,
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata,
        &[],
    )
    .unwrap()
    .id();
    (cone, member)
}

fn row(object: ObjectKey, address: u64) -> ConeMapSymbol {
    ConeMapSymbol {
        cone: object.0,
        member: object.1,
        address,
    }
}

#[test]
fn retained_candidate_comes_from_the_actual_map_not_input_order() {
    let objects = [
        object(ConeIdentity::CORE),
        object(ConeIdentity::SINGLE_FILE),
    ];
    let candidates = BTreeSet::from(objects);
    for winner in objects {
        let map = LinkMap {
            cones: BTreeMap::from([("_shared".into(), vec![row(winner, 0x1000)])]),
            ..LinkMap::default()
        };
        assert_eq!(
            retained(&map, "_shared", &candidates, 0x1000).unwrap(),
            winner
        );
    }
}

#[test]
fn missing_duplicate_foreign_and_displaced_winners_are_rejected() {
    let first = object(ConeIdentity::CORE);
    let second = object(ConeIdentity::SINGLE_FILE);
    let candidates = BTreeSet::from([first, second]);
    for (rows, expected) in [
        (vec![], "no unique retained object"),
        (
            vec![row(first, 0x1000), row(second, 0x1000)],
            "no unique retained object",
        ),
        (
            vec![row((first.0, second.1), 0x1000)],
            "outside its candidates",
        ),
        (
            vec![row((second.0, first.1), 0x1000)],
            "outside its candidates",
        ),
        (
            vec![row(first, 0x1004)],
            "address differs from final symbol table",
        ),
    ] {
        let map = LinkMap {
            cones: BTreeMap::from([("_shared".into(), rows)]),
            ..LinkMap::default()
        };
        let message = retained(&map, "_shared", &candidates, 0x1000)
            .unwrap_err()
            .to_string();
        assert!(message.contains(expected), "{message}");
    }
    assert!(retained(&LinkMap::default(), "_shared", &candidates, 0x1000).is_err());
}
