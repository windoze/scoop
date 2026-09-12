use scoop_identity::{CborIdentityRecord, CoreBuiltinNominal, ExactTypeKey};

use super::*;

fn module_with_box() -> Module {
    let (mut module, _) = module_with_variants(Vec::new());
    let payload = Type::Unit;
    let class = module.classes.alloc(ClassDef {
        modifier: ClassModifier::Final,
        name: "box$U".to_string(),
        type_arguments: Vec::new(),
        representation: ClassRepresentation::Declared {
            fields: vec![Field {
                name: "value".to_string(),
                ty: payload.clone(),
            }],
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    register_test_exact_type(&mut module, &payload);
    module
        .meta
        .boxed_types
        .push(BoxedType::for_source_nominal(payload, class, &exact).unwrap());
    install_generated_exact_types(&mut module);
    module
}

#[test]
fn boxed_value_identity_and_class_shape_validate_together() {
    module_with_box().validate().unwrap();
}

#[test]
fn boxed_value_payload_field_corruption_is_rejected() {
    let mut module = module_with_box();
    let class = module.meta.boxed_types[0].class();
    module.classes[class].declared_fields_mut()[0].ty = Type::Boolean;
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::BoxedValue { boxed: 0 },
            kind: MirValidationErrorKind::InvalidBoxedValue {
                reason: "the generated box has an invalid payload field"
            }
        })
    ));
}

#[test]
fn duplicate_boxed_exact_payload_is_rejected() {
    let mut module = module_with_box();
    let duplicate = module.meta.boxed_types[0].clone();
    module.meta.boxed_types.push(duplicate);
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::BoxedValue { boxed: 1 },
            kind: MirValidationErrorKind::InvalidBoxedValue {
                reason: "the same exact payload is boxed more than once"
            }
        })
    ));
}

#[test]
fn every_generated_box_requires_an_exact_type_identity() {
    let mut module = module_with_box();
    module.meta.generated_exact_types = GeneratedExactTypeIdentities::default();
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::GeneratedExactType { entry: 0 },
            kind: MirValidationErrorKind::InvalidGeneratedExactType {
                reason: "a MIR-generated nominal has no exact-type identity",
            },
        })
    );
}
