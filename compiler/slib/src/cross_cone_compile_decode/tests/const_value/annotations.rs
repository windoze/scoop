use super::*;
use scoop_hir::{
    AnnotatedTargetV1, AnnotationApplicationV1, AnnotationDeclarationV1, AnnotationParameterV1,
    AnnotationTargetV1, CanonicalAnnotationValueV1::Scalar, CanonicalAnnotationsV1,
    DeclaredVisibilityV1,
};
use scoop_identity::PersistentAnnotationId;

#[derive(Clone, Copy)]
enum Case {
    Valid,
    Arity,
    ArgumentType,
    DefaultType,
    GlobalTarget,
    MissingDeclaration,
}

fn annotated(case: Case) -> ConstSurface {
    let mut fixture = ConstSurface::new(ConstSurfaceCase::IntrinsicBoolean);
    let original = fixture.artifact();
    let front = validate_until_source_interfaces(&original);
    let mut section = front.hir_interface().clone();
    let source = section.constants().records()[0].definition_origin().clone();
    let SignatureTypeKey::Nominal(value_type) = section.constants().records()[0].value_type()
    else {
        panic!("the fixture has a scalar nominal constant");
    };
    let value_type = *value_type;
    let identity =
        CborIdentityRecord::<PersistentAnnotationId, _>::from_key(SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                fixture.cone.identity(),
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Flag").unwrap(),
            SourceNominalKind::AnnotationClass,
            0,
        ))
        .unwrap();
    let boolean = Scalar(CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True));
    let declaration = AnnotationDeclarationV1 {
        annotation: identity.id(),
        parameters: vec![AnnotationParameterV1 {
            name: CanonicalIdentifier::new("value").unwrap(),
            value_type: SignatureTypeKey::Nominal(value_type),
            default: Some(if matches!(case, Case::DefaultType) {
                Scalar(CanonicalConstValueV1::String("wrong".into()))
            } else {
                boolean.clone()
            }),
        }],
        visibility: DeclaredVisibilityV1::Internal,
        definition_origin: source.clone(),
    };
    let target = if matches!(case, Case::GlobalTarget) {
        let PropertyOwner::Property(id) = section.property_interfaces().records()[0].declaration()
        else {
            panic!("ordinary property")
        };
        AnnotationTargetV1::Property(id)
    } else {
        AnnotationTargetV1::Nominal(SourceNominalId::Concrete(value_type))
    };
    let arguments = match case {
        Case::Arity => vec![],
        Case::ArgumentType => vec![Scalar(CanonicalConstValueV1::String("wrong".into()))],
        _ => vec![boolean],
    };
    section.set_annotations(
        CanonicalAnnotationsV1::try_new(
            if matches!(case, Case::MissingDeclaration) {
                vec![]
            } else {
                vec![declaration]
            },
            vec![AnnotatedTargetV1 {
                target,
                annotations: vec![AnnotationApplicationV1 {
                    annotation: identity.id(),
                    arguments,
                    definition_origin: source,
                }],
            }],
        )
        .unwrap(),
    );
    fixture.foundation.set_annotations(vec![identity]).unwrap();
    fixture.interface = encode(&section.index_for_wire().unwrap()).unwrap();
    fixture
}

#[test]
fn annotation_wire_round_trip_preserves_the_actual_declaration_and_typed_values() {
    let fixture = annotated(Case::Valid);
    let bytes = fixture.artifact();
    let front = validate_until_source_interfaces(&bytes)
        .validate_const_values(vec![])
        .unwrap();
    let annotations = front.hir_interface().annotations();
    assert_eq!(annotations.declarations().len(), 1);
    assert_eq!(annotations.targets().len(), 1);
    assert_eq!(
        annotations.declarations()[0].annotation,
        annotations.targets()[0].annotations[0].annotation
    );
    assert_eq!(
        annotations.targets()[0].annotations[0].arguments,
        vec![Scalar(CanonicalConstValueV1::Boolean(
            CanonicalBooleanV1::True
        ))]
    );
}

#[test]
fn annotation_reader_rejects_mismatched_arguments_defaults_and_targets() {
    for (case, message) in [
        (
            Case::Arity,
            "annotation argument count differs from its declaration",
        ),
        (
            Case::ArgumentType,
            "annotation argument has a different type",
        ),
        (Case::DefaultType, "annotation default has a different type"),
        (
            Case::GlobalTarget,
            "annotation target is not a retained source declaration",
        ),
        (
            Case::MissingDeclaration,
            "annotation reference has no declaration",
        ),
    ] {
        let bytes = annotated(case).artifact();
        let Err(CrossConeHirConstSurfaceError::Annotations(error)) =
            validate_until_source_interfaces(&bytes).validate_const_values(vec![])
        else {
            panic!("malformed annotation must fail at the source metadata boundary");
        };
        assert_eq!(error.to_string(), message);
    }
}
