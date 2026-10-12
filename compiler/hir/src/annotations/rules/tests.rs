use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn annotation_rules_wire_retains_target_sets_and_rejects_unknown_bits() {
    let mut restricted = AnnotationRules::only(AnnotationTargetKind::PropertyGetter);
    assert!(restricted.insert(AnnotationTargetKind::ValueParameter));
    assert!(!restricted.insert(AnnotationTargetKind::ValueParameter));
    assert!(!restricted.permits(AnnotationTargetKind::Property));
    restricted.repeatable = true;
    for rules in [
        restricted,
        AnnotationRules::default(),
        AnnotationRules::none(),
    ] {
        assert_eq!(
            decode_canonical::<AnnotationRules>(&encode(&rules).unwrap()).unwrap(),
            rules
        );
    }
    for bytes in [&[0xa2, 1, 0x19, 0x10, 0, 2, 0][..], &[0xa2, 1, 0, 2, 2][..]] {
        assert_eq!(
            decode_canonical::<AnnotationRules>(bytes)
                .unwrap_err()
                .kind(),
            &WireErrorKind::IntegerOutOfRange
        );
    }
}
