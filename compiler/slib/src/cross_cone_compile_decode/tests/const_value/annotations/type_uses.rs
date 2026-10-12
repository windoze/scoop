use super::*;
use scoop_hir::{
    AnnotatedTypeUseV1, AnnotationTypeSiteV1, AnnotationTypeSlot, AnnotationTypeUseId,
    TypeAnnotation, TypeAnnotationNode, TypeUseStep,
};
use scoop_identity::SourceSpan;

#[derive(Clone, Copy)]
enum TypeCase {
    Valid,
    Repeatable,
    NonRepeatable,
    Path,
    Binder,
    Excluded,
    Argument,
    Slot,
}

fn type_annotated(case: TypeCase) -> ConstSurface {
    let mut fixture = annotated(Case::Valid);
    let original = fixture.artifact();
    let front = validate_until_source_interfaces(&original);
    let mut section = front.hir_interface().clone();
    let table = section.annotations();
    let mut declarations = table.declarations().to_vec();
    declarations[0].rules.repeatable = matches!(case, TypeCase::Repeatable);
    if matches!(case, TypeCase::Excluded) {
        declarations[0].rules =
            scoop_hir::AnnotationRules::only(scoop_hir::AnnotationTargetKind::Class);
    }
    let application = &table.targets()[0].annotations[0];
    let identity = AnnotationTypeUseId {
        source: application.definition_origin.origin().source().clone(),
        span: SourceSpan::new(0, 1).unwrap(),
    };
    let application = TypeAnnotation {
        annotation: application.annotation,
        arguments: if matches!(case, TypeCase::Argument) {
            vec![Scalar(CanonicalConstValueV1::String("wrong".into()))]
        } else {
            application.arguments.clone()
        },
        source: identity.source.clone(),
        span: identity.span,
    };
    let ty = if matches!(case, TypeCase::Binder) {
        SignatureTypeKey::Binder { depth: 0, index: 1 }
    } else {
        section.constants().records()[0].value_type().clone()
    };
    let usage = AnnotatedTypeUseV1 {
        identity: identity.clone(),
        ty: ty.into(),
        binder_count: u32::from(matches!(case, TypeCase::Binder)),
        nodes: vec![TypeAnnotationNode {
            path: if matches!(case, TypeCase::Path) {
                vec![TypeUseStep::Argument(0)]
            } else {
                vec![]
            },
            annotations: if matches!(case, TypeCase::Repeatable | TypeCase::NonRepeatable) {
                vec![application.clone(), application]
            } else {
                vec![application]
            },
        }],
    };
    let sites = if matches!(case, TypeCase::Slot) {
        vec![AnnotationTypeSiteV1 {
            target: table.targets()[0].target,
            slot: AnnotationTypeSlot::Value,
            type_use: identity,
        }]
    } else {
        vec![]
    };
    section.set_annotations(
        CanonicalAnnotationsV1::try_new(declarations, table.targets().to_vec(), vec![usage], sites)
            .unwrap(),
    );
    fixture.interface = encode(&section.index_for_wire().unwrap()).unwrap();
    fixture
}

#[test]
fn type_annotation_wire_preserves_usage_identity_paths_and_ordered_values() {
    for (case, count) in [(TypeCase::Valid, 1), (TypeCase::Repeatable, 2)] {
        let fixture = type_annotated(case);
        let bytes = fixture.artifact();
        let front = validate_until_source_interfaces(&bytes)
            .validate_const_values(vec![])
            .unwrap();
        let usage = &front.hir_interface().annotations().type_uses()[0];
        assert_eq!(usage.identity.source.cone(), fixture.cone.identity());
        assert_eq!(usage.identity.span, SourceSpan::new(0, 1).unwrap());
        assert_eq!(usage.nodes[0].annotations.len(), count);
        assert_eq!(
            usage.nodes[0].annotations[0].arguments,
            [Scalar(CanonicalConstValueV1::Boolean(
                CanonicalBooleanV1::True
            ))]
        );
    }
}

#[test]
fn type_annotation_reader_rejects_invalid_types_positions_values_and_rules() {
    for (case, message) in [
        (
            TypeCase::Path,
            "type annotation path does not exist in its type use",
        ),
        (
            TypeCase::Binder,
            "annotated type has an invalid signature or binder",
        ),
        (
            TypeCase::Excluded,
            "annotation declaration excludes type uses",
        ),
        (
            TypeCase::NonRepeatable,
            "repeated non-repeatable annotation on one type position",
        ),
        (
            TypeCase::Argument,
            "annotation argument has a different type",
        ),
        (
            TypeCase::Slot,
            "type annotation slot is not part of its declaration",
        ),
    ] {
        let bytes = type_annotated(case).artifact();
        let Err(CrossConeHirConstSurfaceError::Annotations(error)) =
            validate_until_source_interfaces(&bytes).validate_const_values(vec![])
        else {
            panic!("invalid type annotations must fail at the reader boundary");
        };
        assert_eq!(error.to_string(), message);
    }
}
