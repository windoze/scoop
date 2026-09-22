use super::*;
use crate::*;

fn fixture() -> ProviderFixture {
    structure(
        coordinate("invalid-native"),
        "Payload",
        false,
        |owner| vec![pointer(nominal(owner))],
        policy(),
    )
}

#[test]
fn native_producer_rejects_missing_canonical_fields_and_value_shapes() {
    let mut fixture = fixture();
    fixture.foundation.set_fields(vec![]).unwrap();
    with_world(&[&fixture], &[], |world| {
        assert!(matches!(
            world.native_boundary_type_definition(owner(&fixture), Ok),
            Err(Error::MissingDependencyField { .. })
        ));
    });
    let mut fixture = self::fixture();
    fixture.interface = ProviderFixture::empty(fixture.coordinate.clone()).interface;
    with_world(&[&fixture], &[], |world| {
        assert!(matches!(
            world.native_boundary_type_definition(owner(&fixture), Ok),
            Err(Error::MissingDependencyShape { .. })
        ));
    });
}

#[test]
fn native_producer_rejects_missing_canonical_variant_and_payload_keys() {
    for missing_variant in [false, true] {
        let mut fixture = enumeration(
            coordinate("invalid-enum-native"),
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
        );
        if missing_variant {
            fixture.foundation.set_enum_variants(vec![]).unwrap();
            fixture.foundation.set_enum_variant_fields(vec![]).unwrap();
        } else {
            fixture.foundation.set_enum_variant_fields(vec![]).unwrap();
        }
        let result = try_with_world(&[&fixture], &[], |world| {
            world.native_boundary_type_definition(owner(&fixture), Ok)
        });
        if missing_variant {
            assert!(matches!(
                result,
                Err(ImportedSemanticWorldBuildError::MissingEntityIdentity { .. })
            ));
        } else {
            assert!(matches!(
                result,
                Ok(Err(Error::MissingDependencyVariantField { .. }))
            ));
        }
    }
}

#[test]
fn native_producer_rejects_wrong_field_owner_kind_and_binder() {
    let mut fixture = fixture();
    let wrong = structure(
        fixture.coordinate.clone(),
        "Other",
        false,
        |owner| vec![pointer(nominal(owner))],
        policy(),
    );
    let wrong_shape = wrong.interface.nominal_interfaces().records()[0]
        .source_shape()
        .clone();
    let NominalSourceShapeV1::Struct(shape) = &wrong_shape else {
        unreachable!()
    };
    let field = shape.fields()[0].field();
    let wrong_field = wrong
        .foundation
        .field_by_bytes(field.as_array())
        .unwrap()
        .1
        .clone();
    fixture
        .foundation
        .set_fields(vec![CborIdentityRecord::from_key(wrong_field).unwrap()])
        .unwrap();
    // Register both source owners so the failure is the shape relation itself.
    fixture
        .foundation
        .set_types(vec![
            CborIdentityRecord::from_key(fixture.outer_key.clone().unwrap()).unwrap(),
            CborIdentityRecord::from_key(wrong.outer_key.clone().unwrap()).unwrap(),
        ])
        .unwrap();
    set_shape(&mut fixture, wrong_shape);
    with_world(&[&fixture], &[], |world| {
        assert!(matches!(
            world.native_boundary_type_definition(owner(&fixture), Ok),
            Err(Error::InvalidDefinition(
                NativeBoundaryDefinitionError::FieldOwnerMismatch
            ))
        ));
    });
    let mut fixture = self::fixture();
    set_shape(&mut fixture, NominalSourceShapeV1::Class);
    with_world(&[&fixture], &[], |world| {
        assert!(matches!(
            world.native_boundary_type_definition(owner(&fixture), Ok),
            Err(Error::InvalidDefinition(
                NativeBoundaryDefinitionError::ShapeKindMismatch
            ))
        ));
    });
    let fixture = structure(
        coordinate("bad-binder-native"),
        "Payload",
        false,
        |_| vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
        policy(),
    );
    with_world(&[&fixture], &[], |world| {
        assert!(matches!(
            world.native_boundary_type_definition(owner(&fixture), Ok),
            Err(Error::InvalidDefinition(
                NativeBoundaryDefinitionError::BinderIndexOutOfRange { .. }
            ))
        ));
    });
}

#[test]
fn native_producer_reuses_private_witnesses_and_rejects_divergent_public_shapes() {
    let mut fixture = fixture();
    let record = with_world(&[&fixture], &[], |world| {
        world
            .native_boundary_type_definition(owner(&fixture), Ok)
            .unwrap()
    });
    fixture
        .foundation
        .set_native_boundary_types(vec![record.clone()])
        .unwrap();
    let source = fixture.interface.clone();
    fixture.interface = ProviderFixture::empty(fixture.coordinate.clone()).interface;
    with_world(&[], &[&fixture], |world| {
        assert!(world.nominal(fixture.outer.unwrap()).is_none());
        assert_eq!(
            world
                .native_boundary_type_definition(owner(&fixture), Ok)
                .unwrap(),
            record
        );
    });
    fixture.interface = source;
    let Shape::Struct { fields, .. } = record.shape() else {
        unreachable!()
    };
    let wrong = Record::new(
        fixture.outer_key.as_ref().unwrap(),
        &[0],
        Shape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
            fields: fields.clone(),
        },
    )
    .unwrap();
    fixture
        .foundation
        .set_native_boundary_types(vec![wrong])
        .unwrap();
    with_world(&[&fixture], &[], |world| {
        assert!(matches!(
            world.native_boundary_type_definition(owner(&fixture), Ok),
            Err(Error::DependencyDefinitionMismatch { .. })
        ));
    });
}
