use super::*;
use scoop_hir::{CanonicalAnnotationValueV1, CanonicalConstValueV1};

#[test]
fn annotation_values_require_the_declared_container_and_element_payload() {
    let kind = AnnotationParameterKind::Array(AnnotationAtomKind::Scalar(
        CanonicalConstValueKindV1::Boolean,
    ));
    let boolean = CanonicalConstValueV1::Boolean(true.into());
    let array =
        |element_type, elements: Vec<CanonicalConstValueV1>| CanonicalAnnotationValueV1::Array {
            element_type: scoop_hir::AnnotationElementTypeV1::Scalar(element_type),
            elements: elements
                .into_iter()
                .map(|value| scoop_hir::AnnotationElementV1 {
                    value: CanonicalAnnotationValueV1::Scalar(value),
                    source: scoop_identity::SourceIdentity::single_file(),
                    span: scoop_identity::SourceSpan::new(1, 2).unwrap(),
                })
                .collect(),
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
        !AnnotationParameterKind::Atom(AnnotationAtomKind::Scalar(
            CanonicalConstValueKindV1::Boolean
        ))
        .accepts(&array(CanonicalConstValueKindV1::Boolean, vec![]))
    );
}
