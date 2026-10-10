use super::*;
use scoop_hir::CanonicalConstValueV1;

#[test]
fn annotation_values_require_the_declared_container_and_element_payload() {
    let kind = AnnotationParameterKind::Array(CanonicalConstValueKindV1::Boolean);
    let boolean = CanonicalConstValueV1::Boolean(true.into());
    let array = |element_type, elements| CanonicalAnnotationValueV1::Array {
        element_type,
        elements,
    };
    assert!(kind.accepts(&array(CanonicalConstValueKindV1::Boolean, vec![])));
    assert!(kind.accepts(&array(
        CanonicalConstValueKindV1::Boolean,
        vec![boolean.clone()]
    )));
    assert!(!kind.accepts(&array(CanonicalConstValueKindV1::String, vec![])));
    assert!(!kind.accepts(&array(
        CanonicalConstValueKindV1::Boolean,
        vec![
            boolean.clone(),
            CanonicalConstValueV1::String("wrong".into())
        ]
    )));
    assert!(!kind.accepts(&CanonicalAnnotationValueV1::Scalar(boolean)));
    assert!(
        !AnnotationParameterKind::Scalar(CanonicalConstValueKindV1::Boolean)
            .accepts(&array(CanonicalConstValueKindV1::Boolean, vec![]))
    );
}
