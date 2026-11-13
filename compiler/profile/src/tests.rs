use scoop_wire::Digest256;

use super::*;

const TARGET: &str = "aarch64-apple-darwin";

fn key(number: u8) -> BodyKey {
    BodyKey {
        body: serde_json::from_str(&format!("\"{}\"", format!("{number:02x}").repeat(32))).unwrap(),
        fingerprint: Digest256::from_array([number; 32]).into(),
    }
}

fn body(key: BodyKey, kinds: &[PositionKind]) -> BodyLayout {
    BodyLayout {
        key,
        name: "display name does not identify a body".into(),
        positions: kinds.iter().map(|&kind| Position { kind }).collect(),
    }
}

fn at(body: BodyKey, position: u32) -> Location {
    Location { body, position }
}

fn counter(body: BodyKey, position: u32, inlined_at: Vec<Location>) -> Counter {
    Counter {
        location: at(body, position),
        inlined_at: inlined_at.into_iter().map(InlineOrigin::Call).collect(),
    }
}

fn inline_map() -> ProfileMap {
    let root = key(1);
    let callee = key(2);
    ProfileMap {
        schema: SCHEMA,
        target: TARGET.into(),
        bodies: vec![
            body(
                root,
                &[PositionKind::Entry, PositionKind::Call, PositionKind::Call],
            ),
            body(callee, &[PositionKind::Entry, PositionKind::Call]),
        ],
        records: vec![Record {
            name: "physical".into(),
            hash: 99,
            counters: vec![
                counter(root, 0, vec![]),
                counter(callee, 0, vec![at(root, 1)]),
                counter(callee, 1, vec![at(root, 1)]),
                counter(callee, 0, vec![at(root, 2)]),
                counter(callee, 1, vec![at(root, 2)]),
            ],
        }],
    }
}

fn counts(values: &[u64]) -> IndexedRecord {
    IndexedRecord {
        name: b"physical".to_vec(),
        hash: 99,
        counts: values.to_vec(),
    }
}

#[test]
fn inline_copies_pair_entries_and_calls_before_aggregation() {
    let map = inline_map();
    let feedback =
        Feedback::from_records(TARGET, &[map.clone(), map], &[counts(&[1, 10, 30, 20, 2])])
            .unwrap();
    assert_eq!(
        feedback.frequency(at(key(2), 1)),
        Ok(Frequency {
            calls: 32,
            entries: 30,
            partial: false
        })
    );
    assert_eq!(
        feedback.count(at(key(2), 0)),
        Count::Observed {
            value: 30,
            partial: false
        }
    );
}

#[test]
fn missing_inline_counter_does_not_borrow_an_unrelated_denominator() {
    let mut map = inline_map();
    map.records[0].counters.pop();
    let feedback = Feedback::from_records(TARGET, &[map], &[counts(&[1, 10, 30, 20])]).unwrap();
    assert_eq!(
        feedback.frequency(at(key(2), 1)),
        Ok(Frequency {
            calls: 30,
            entries: 10,
            partial: true
        })
    );
    assert_eq!(
        feedback.count(at(key(2), 0)),
        Count::Observed {
            value: 30,
            partial: false
        }
    );
    assert_eq!(
        feedback.count(at(key(2), 1)),
        Count::Observed {
            value: 30,
            partial: true
        }
    );
}

#[test]
fn zeros_absence_and_changed_bodies_remain_distinct() {
    let map = inline_map();
    let zero = Feedback::from_records(
        TARGET,
        std::slice::from_ref(&map),
        &[counts(&[0, 0, 0, 0, 0])],
    )
    .unwrap();
    assert_eq!(
        zero.count(at(key(2), 1)),
        Count::Observed {
            value: 0,
            partial: false
        }
    );
    assert_eq!(zero.frequency(at(key(2), 1)), Err(MissingCount::ZeroEntry));
    let missing = Feedback::from_records(TARGET, &[map], &[]).unwrap();
    assert_eq!(
        missing.count(at(key(2), 1)),
        Count::Missing(MissingCount::MissingRecord)
    );
    assert_eq!(
        missing.count(at(key(9), 1)),
        Count::Missing(MissingCount::MissingBody)
    );
    let changed = BodyKey {
        fingerprint: key(9).fingerprint,
        ..key(2)
    };
    assert_eq!(
        missing.count(at(changed, 1)),
        Count::Missing(MissingCount::BodyChanged)
    );
}

#[test]
fn partial_records_and_overflow_are_reported_without_wrapping() {
    let mut map = inline_map();
    let mut second = map.records[0].clone();
    second.name = "unlinked".into();
    map.records.push(second);
    let feedback =
        Feedback::from_records(TARGET, &[map.clone()], &[counts(&[1, 10, 30, 20, 2])]).unwrap();
    assert_eq!(
        feedback.frequency(at(key(2), 1)),
        Ok(Frequency {
            calls: 32,
            entries: 30,
            partial: true
        })
    );
    assert_eq!(
        feedback.count(at(key(2), 1)),
        Count::Observed {
            value: 32,
            partial: true
        }
    );
    let overflow =
        Feedback::from_records(TARGET, &[map], &[counts(&[1, 1, u64::MAX, 1, 1])]).unwrap();
    assert_eq!(
        overflow.frequency(at(key(2), 1)),
        Err(MissingCount::Overflow)
    );
    assert_eq!(
        overflow.count(at(key(2), 1)),
        Count::Missing(MissingCount::Overflow)
    );
}

#[test]
fn malformed_maps_fail_at_the_input_boundary() {
    let map = inline_map();
    let mut changed = map.clone();
    changed.records[0].counters.swap(0, 1);
    assert!(Feedback::from_records(TARGET, &[map.clone(), changed], &[]).is_err());
    assert!(Feedback::from_records(TARGET, std::slice::from_ref(&map), &[counts(&[1])]).is_err());
    let mut invalid = map.clone();
    invalid.records[0].counters[1].inlined_at[0] = InlineOrigin::Call(at(key(1), 0));
    assert!(Feedback::from_records(TARGET, &[invalid], &[]).is_err());
    let mut invalid = map.clone();
    let duplicate = invalid.records[0].counters[0].clone();
    invalid.records[0].counters.push(duplicate);
    assert!(Feedback::from_records(TARGET, &[invalid], &[]).is_err());
    let mut invalid = map;
    invalid.records[0].counters[0].location.position = 100;
    assert!(Feedback::from_records(TARGET, &[invalid], &[]).is_err());
    assert!(serde_json::from_str::<BodyId>("\"你好\"").is_err());
}
