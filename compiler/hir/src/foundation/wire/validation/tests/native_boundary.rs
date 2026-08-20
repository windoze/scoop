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
                    let validated = decoded.validate(&current, &mut identities).unwrap();
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
            decoded.validate(&current, &mut identities),
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
        let error = decoded.validate(&current, &mut identities).err().unwrap();
        assert!(
            matches!(error,
                HirFoundationValidationError::NativeBoundaryShapeCoverage(NativeBoundaryShapeCoverageError::MissingStructField { .. }) if !enumeration
            ) || matches!(error,
                HirFoundationValidationError::NativeBoundaryShapeCoverage(NativeBoundaryShapeCoverageError::MissingEnumVariant { .. }) if enumeration
            )
        );
    }
}
