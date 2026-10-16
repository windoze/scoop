use super::*;

pub(crate) mod support;
use support::{Corruption, Fixture};

#[test]
fn reads_actual_stackmap_records_and_return_addresses() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified =
        read_scoop_stackmaps_v1(fixture.builtins, fixture.semantic_plan, &objects).unwrap();

    assert_eq!(
        verified.producer(),
        scoop_identity::ConeIdentity::SINGLE_FILE
    );
    assert_eq!(verified.records().len(), 2);
    assert!(
        verified
            .records()
            .windows(2)
            .all(|pair| pair[0].normalized().canonical().site()
                < pair[1].normalized().canonical().site())
    );
    assert_eq!(
        verified
            .records()
            .iter()
            .map(|record| record.normalized().canonical().site())
            .collect::<Vec<_>>(),
        verified
            .semantic_plan()
            .sites()
            .iter()
            .map(|site| site.site())
            .collect::<Vec<_>>()
    );
    assert!(verified.records().iter().all(|record| {
        record.member() == fixture.member
            && record.normalized().canonical().root_pair_count() == 0
            && record.normalized().canonical().locations().len() == 3
    }));
    let mut return_pcs = verified
        .records()
        .iter()
        .map(ScoopStackmapRecordV1::object_return_pc)
        .collect::<Vec<_>>();
    return_pcs.sort_unstable();
    assert_eq!(return_pcs, [12, 16]);
}

#[test]
fn reports_an_unknown_safepoint_reference() {
    let fixture = Fixture::new(Corruption::UnknownSafepoint);
    let member = fixture.member;
    let objects = [ScoopLirObjectCandidateV1::new(
        member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        read_scoop_stackmaps_v1(fixture.builtins, fixture.semantic_plan, &objects),
        Err(ScoopStackmapReadError::UnexpectedSafepointId {
            member: actual,
            ..
        }) if actual == member
    ));
}
