use super::*;
use crate::DecodedExportDefinitionSourceV1;
use scoop_wire::{ResourceKind, WireErrorKind};

#[test]
fn inline_body_origins_check_source_leaf_length_before_resolution() {
    let f = Fixture::new();
    let locals = CanonicalTemplateLocalTableV1::try_new(vec![]).unwrap();
    let body =
        ExportDefaultBodyV1::try_new(vec![], expression(&f, DefaultExpressionKindV1::UnitLiteral))
            .unwrap();
    let input = decoded(&body, &locals);
    let length = f.origin().origin().source().logical_path().as_str().len() as u64;
    let mut shared = BudgetMeter::new(DecodeLimits {
        semantic_leaf_bytes: length - 1,
        ..DecodeLimits::default()
    });
    let error = input.charge_resolution(&locals, &mut shared).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticLeafBytes,
            ..
        }
    ));
    let mut enough = BudgetMeter::new(DecodeLimits {
        semantic_leaf_bytes: length,
        ..DecodeLimits::default()
    });
    input.charge_resolution(&locals, &mut enough).unwrap();
}

#[test]
fn inline_origin_preflight_checks_nested_depth_and_each_copy_in_one_budget() {
    let f = Fixture::new();
    let input: DecodedExportDefinitionSourceV1 =
        decode_canonical(&encode(&f.origin()).unwrap(), DecodeLimits::default()).unwrap();
    let mut shallow = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 2,
        ..DecodeLimits::default()
    });
    let error = charge(&input, None, 1, &mut shallow).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticRecursion,
            ..
        }
    ));
    let mut one = meter();
    charge(&input, None, 1, &mut one).unwrap();
    let mut two = meter();
    charge(&input, None, 2, &mut two).unwrap();
    assert!(two.usage().decoded_nodes > one.usage().decoded_nodes);
    assert!(two.usage().owned_bytes > one.usage().owned_bytes);
    assert!(two.usage().validation_work_units > one.usage().validation_work_units);
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: one.usage().validation_work_units * 2 - 1,
        ..DecodeLimits::default()
    });
    charge(&input, None, 1, &mut shared).unwrap();
    assert!(charge(&input, None, 1, &mut shared).is_err());
}
