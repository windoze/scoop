use super::*;
use scoop_wire::{WireErrorKind, decode_canonical, encode};

fn directory() -> HostPathCarrier {
    HostPathCarrier::from_path(std::path::Path::new("observations")).unwrap()
}

#[test]
fn all_nonempty_stage_sets_round_trip_in_stage_order() {
    for bits in 1..16 {
        let stages = StageDumpSet::new(
            &StageDumpKindV1::ALL
                .into_iter()
                .enumerate()
                .filter(|(index, _)| bits & (1 << index) != 0)
                .map(|(_, stage)| stage)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let policy = StageDumpPolicyV1::Files {
            stages,
            directory: directory(),
        };
        let decoded = decode_canonical::<DecodedStageDumpPolicyV1>(&encode(&policy).unwrap())
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(decoded, policy);
    }
}

#[test]
fn empty_repeated_and_reordered_stage_sets_are_rejected() {
    use StageDumpKindV1::*;
    for stages in [&[][..], &[Ast, Ast], &[Lir, Mir]] {
        assert_eq!(
            StageDumpSet::new(stages),
            Err(ProtocolValidationError::InvalidDumpStages)
        );
    }
    for stages in [vec![], vec![1, 1], vec![4, 2], vec![5]] {
        let raw = DecodedStageDumpPolicyV1::Files {
            stages,
            directory: decode_canonical::<DecodedHostPathCarrier>(&encode(&directory()).unwrap())
                .unwrap(),
        };
        let bytes = encode(&raw).unwrap();
        assert!(
            decode_canonical::<DecodedStageDumpPolicyV1>(&bytes)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}

#[test]
fn retired_single_stage_tag_is_rejected() {
    let error = decode_canonical::<DecodedStageDumpPolicyV1>(&[0xa2, 0, 2, 1, 1]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 2 });
}
