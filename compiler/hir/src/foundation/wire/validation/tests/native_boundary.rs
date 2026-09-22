use super::*;
use crate::{
    NativeBoundaryFieldDefinition, NativeBoundaryVariantDefinition,
    NativeBoundaryVariantFieldDefinition,
};
use scoop_identity::*;

mod support;
use support::Fixture as BoundaryFixture;

#[test]
fn native_boundary_reads_actual_local_and_external_declarations_through_one_graph() {
    let ordinary = ConeCoordinate::new("example", "native-provider", "1.0.0").unwrap();
    let consumer = ConeCoordinate::new("example", "native-consumer", "1.0.0").unwrap();
    for provider in [ConeCoordinate::reserved_core(), ordinary] {
        for generic in [false, true] {
            for enumeration in [false, true] {
                let fixture =
                    BoundaryFixture::new(provider.identity().unwrap(), generic, enumeration);
                for current in [ConeCoordinate::reserved_core(), consumer.clone()] {
                    if current == provider {
                        continue;
                    }
                    let input = fixture.input();
                    let decoded = decode(&input);
                    let mut identities = fixture.graph(&decoded, &current, true);
                    let validated = decoded
                        .validate(&current, &mut identities, &mut meter())
                        .unwrap();
                    assert_eq!(encode(&validated).unwrap(), encode(&input).unwrap());
                    assert_eq!(validated.counts().fields, 0);
                    assert_eq!(validated.counts().enum_variants, 0);
                    assert_eq!(validated.counts().native_boundary_types, 1);
                }
            }
        }
    }
}

#[test]
fn native_boundary_requires_dependency_keys_and_complete_field_variant_coverage() {
    let current = ConeCoordinate::reserved_core();
    let provider = ConeCoordinate::new("example", "boundary", "1.0.0").unwrap();
    for enumeration in [false, true] {
        let fixture = BoundaryFixture::new(provider.identity().unwrap(), false, enumeration);
        let input = fixture.input();
        let decoded = decode(&input);
        let mut identities = fixture.graph(&decoded, &current, false);
        assert!(matches!(
            decoded.validate(&current, &mut identities, &mut meter()),
            Err(HirFoundationValidationError::NativeBoundaryType {
                index: 0,
                error: NativeBoundaryResolutionError::Reference(
                    IdentityReferenceError::KeyUnavailable { .. }
                )
            })
        ));

        let mut input = fixture.input();
        input
            .set_native_boundary_types(vec![fixture.without_members()])
            .unwrap();
        let decoded = decode(&input);
        let mut identities = fixture.graph(&decoded, &current, true);
        let error = decoded
            .validate(&current, &mut identities, &mut meter())
            .err()
            .unwrap();
        assert!(
            matches!(error,
                HirFoundationValidationError::NativeBoundaryShapeCoverage(NativeBoundaryShapeCoverageError::MissingStructField { .. }) if !enumeration
            ) || matches!(error,
                HirFoundationValidationError::NativeBoundaryShapeCoverage(NativeBoundaryShapeCoverageError::MissingEnumVariant { .. }) if enumeration
            )
        );
    }
}

#[test]
fn native_boundary_graph_queries_keep_the_callers_continuous_budget() {
    let current = ConeCoordinate::reserved_core();
    let provider = ConeCoordinate::new("example", "budget", "1.0.0").unwrap();
    let fixture = BoundaryFixture::new(provider.identity().unwrap(), false, true);
    let input = fixture.input();
    let mut identities = fixture.graph(&decode(&input), &current, true);
    let run = |identities: &mut ValidatedIdentityGraph, meter: &mut BudgetMeter| {
        decode(&input).validate(&current, identities, meter)
    };
    let mut measured = meter();
    run(&mut identities, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    run(&mut identities, &mut shared).unwrap();
    assert!(run(&mut identities, &mut shared).is_err());
}
