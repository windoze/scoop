use super::*;
use crate::cross_cone_interface::expression_test_support::Fixture;
use scoop_identity::{Effect, StructuralDefinitionSiteRole, StructuralPathSegment};
use scoop_wire::{DecodeLimits, ResourceKind};

fn check(error: WireError, expected: ResourceKind) {
    assert!(
        matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected)
    );
}

#[test]
fn comparison_checks_pattern_literal_text_in_its_metadata_attachment() {
    let fixture = Fixture::new();
    let value_type = fixture.value_type();
    let pattern = DefaultPatternV1::literal(
        CanonicalConstValueV1::String("x".repeat(128)),
        DefaultLiteralEqualityV1::Ordinary {
            target: fixture.callable(),
        },
        value_type.clone(),
    );
    let origin = fixture.origin();
    let at = WirePath::root().field(5);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_leaf_bytes: 64,
        ..DecodeLimits::default()
    });
    let error = Leaves
        .reference(
            DefaultBodyReferenceOccurrenceV1 {
                target: DefaultBodyReferenceTargetV1::Type(&value_type),
                definition_origin: &origin,
                site: ExportDefaultReferenceOccurrenceSiteV1::BodyType(
                    DefaultBodyProviderTypeSiteV1::PatternSubject,
                ),
                attachment: DefaultBodyReferenceAttachmentV1::Metadata(
                    DefaultBodyReferenceMetadataV1::Pattern(&pattern),
                ),
            },
            &mut meter,
            &at,
        )
        .unwrap_err();
    assert_eq!(error.path(), &at);
    check(error, ResourceKind::SemanticLeafBytes);
}

#[test]
fn comparison_checks_local_function_paths_without_an_expression_index() {
    let fixture = Fixture::new();
    let long = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
        (0..64)
            .map(|n| StructuralPathSegment::new(StructuralDefinitionSiteRole::LocalDeclaration, n)),
    );
    let function = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        long,
        SignatureTypeKey::Function {
            effect: Effect::Ordinary,
            parameters: vec![],
            result: Box::new(fixture.value_type()),
        },
        vec![],
        0,
    )
    .unwrap();
    let origin = fixture.origin();
    let at = WirePath::root().field(5);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 32,
        ..DecodeLimits::default()
    });
    let error = Leaves
        .reference(
            DefaultBodyReferenceOccurrenceV1 {
                target: DefaultBodyReferenceTargetV1::Type(function.function_type()),
                definition_origin: &origin,
                site: ExportDefaultReferenceOccurrenceSiteV1::BodyType(
                    DefaultBodyProviderTypeSiteV1::NestedCallableFunction,
                ),
                attachment: DefaultBodyReferenceAttachmentV1::Metadata(
                    DefaultBodyReferenceMetadataV1::LocalFunction(&function),
                ),
            },
            &mut meter,
            &at,
        )
        .unwrap_err();
    assert_eq!(error.path(), &at);
    check(error, ResourceKind::SemanticRecursion);
}
