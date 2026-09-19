use scoop_identity::{
    LocalValueSelector, SignatureTypeKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

use super::*;
use crate::cross_cone_interface::expression_test_support::Fixture;
use crate::{
    CanonicalBooleanV1, DecodedDefaultCaptureV1, DecodedExportDefaultBodyV1,
    DefaultCallableBodyTypeArgumentsV1, DefaultCaptureV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultLambdaV1, DefaultPatternV1, DefaultStatementKindV1,
    DefaultStatementV1, ExportDefaultBodyV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
};

fn locals(f: &Fixture, selector: LocalValueSelector) -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            selector,
            f.value_type(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(f.origin()),
        )
        .unwrap(),
    ])
    .unwrap()
}
fn expression(f: &Fixture, kind: DefaultExpressionKindV1) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, f.value_type(), f.origin()).unwrap()
}
fn decoded(
    body: &ExportDefaultBodyV1,
    locals: &CanonicalTemplateLocalTableV1,
) -> DecodedExportDefaultBodyV1 {
    decode_canonical(
        &encode(&body.index_locals(&mut locals.clone()).unwrap()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn wide_arrays_check_work_and_depth_before_scheduling_their_elements() {
    let values = vec![0_u32; 4096];
    for limits in [
        DecodeLimits {
            validation_work_units: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
    ] {
        let mut limited = BudgetMeter::new(limits);
        assert!(charge(&values, None, 1, &mut limited).is_err());
        assert_eq!(limited.usage().decoded_edges, 0);
        assert_eq!(limited.usage().decoded_nodes, 1);
    }
    let mut resources = meter();
    charge(&values, None, 1, &mut resources).unwrap();
    assert_eq!(resources.usage().decoded_nodes, values.len() as u64 + 1);
}

#[test]
fn typed_body_preflight_rejects_limits_before_resolution_and_covers_binding_and_capture_metadata() {
    let f = Fixture::new();
    let locals = locals(&f, f.local());
    let lambda = DefaultLambdaV1::try_new(
        f.generated,
        crate::cross_cone_interface::expression_test_support::definition_path(),
        SignatureTypeKey::RawPointer(Box::new(f.value_type())),
        DefaultCallableBodyTypeArgumentsV1::try_explicit(vec![f.value_type()]).unwrap(),
        vec![DefaultCaptureV1::new(f.local(), f.value_type(), f.origin())],
        0,
    )
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        vec![
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::ValDecl {
                    pattern: DefaultPatternV1::binding(f.local()),
                    init: Box::new(expression(&f, DefaultExpressionKindV1::Local(f.local()))),
                },
                f.origin(),
            )
            .unwrap(),
        ],
        expression(&f, DefaultExpressionKindV1::Lambda(lambda)),
    )
    .unwrap();
    let input = decoded(&body, &locals);
    for limits in [
        DecodeLimits {
            semantic_recursion: 3,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_edges: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(
            input
                .charge_resolution(&locals, &mut BudgetMeter::new(limits))
                .is_err()
        );
    }
    let mut resources = meter();
    input.charge_resolution(&locals, &mut resources).unwrap();
    assert!(resources.usage().decoded_nodes > 20);
    assert!(resources.usage().owned_bytes > 0);
    assert_eq!(
        input
            .resolve(&mut f.resolver(), &mut locals.clone())
            .unwrap(),
        body
    );
}

#[test]
fn local_capture_preflight_charges_actual_canonical_selector_paths() {
    let f = Fixture::new();
    let short = locals(&f, f.local());
    let path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::LocalDeclaration, 0),
        (1..32).map(|index| {
            StructuralPathSegment::new(StructuralDefinitionSiteRole::LocalDeclaration, index)
        }),
    );
    let selector = LocalValueSelector::LocalDeclaration { path };
    let long = locals(&f, selector.clone());
    let capture = DefaultCaptureV1::new(selector, f.value_type(), f.origin());
    let encoded = encode(&capture.index_local(&mut long.clone()).unwrap()).unwrap();
    let decoded: DecodedDefaultCaptureV1 =
        decode_canonical(&encoded, DecodeLimits::default()).unwrap();
    let mut short_meter = meter();
    charge(&decoded, Some(&short), 2, &mut short_meter).unwrap();
    let mut long_meter = meter();
    charge(&decoded, Some(&long), 2, &mut long_meter).unwrap();
    assert!(long_meter.usage().logical_heap_bytes > short_meter.usage().logical_heap_bytes);
    assert!(long_meter.usage().validation_work_units > short_meter.usage().validation_work_units);
    let mut bounded = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: short_meter.usage().logical_heap_bytes,
        ..DecodeLimits::default()
    });
    assert!(charge(&decoded, Some(&long), 2, &mut bounded).is_err());
}
