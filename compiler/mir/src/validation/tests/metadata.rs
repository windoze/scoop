use super::*;

fn checked_pair(module: &Module, enum_id: EnumId) -> (MirVariantFieldRef, MirVariantRef) {
    let payload = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let payload = MirVariantFieldRef::new(&module.enums, payload, 0).unwrap();
    let empty = MirVariantRef::new(&module.enums, enum_id, 1).unwrap();
    (payload, empty)
}

#[test]
fn module_validation_rejects_stale_option_core_metadata() {
    let value = Type::Integer(IntegerKind::SIGNED_32);
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Some", vec![value.clone()]),
        variant_def("None", Vec::new()),
    ]);
    module.enums[enum_id].type_arguments = vec![value];
    let (some_payload, none) = checked_pair(&module, enum_id);
    module.option_core.push(
        OptionCore::checked(&module.enums, some_payload, none).expect("valid Option metadata"),
    );
    assert_eq!(module.validate(), Ok(()));

    module.enums[enum_id].variants[0].fields.clear();
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::OptionCore {
                enumeration: enum_id,
            },
            kind: MirValidationErrorKind::InvalidOptionCore,
        })
    );
}

#[test]
fn module_validation_rejects_stale_coroutine_step_shape() {
    let result = Type::Integer(IntegerKind::SIGNED_32);
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Completed", vec![result.clone()]),
        variant_def("Suspended", Vec::new()),
    ]);
    let (completed_payload, suspended) = checked_pair(&module, enum_id);
    let identity = test_step_identity(&result);
    let step = CoroutineStep::checked(
        &module.enums,
        completed_payload,
        suspended,
        result,
        identity,
    )
    .expect("valid CoroutineStep metadata");
    let step_id = module.meta.coroutine_steps.alloc(step);
    assert_eq!(module.validate(), Ok(()));

    module.enums[enum_id].variants[0].fields.push(Field {
        name: "stale".to_string(),
        ty: Type::Boolean,
    });
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::CoroutineStep { step: step_id },
            kind: MirValidationErrorKind::InvalidCoroutineStep,
        })
    );
}

#[test]
fn module_validation_rejects_stale_coroutine_slot_reference() {
    let value = Type::Boolean;
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Empty", Vec::new()),
        variant_def("Value", vec![value.clone()]),
    ]);
    let empty = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let value_variant = MirVariantRef::new(&module.enums, enum_id, 1).unwrap();
    let value_payload = MirVariantFieldRef::new(&module.enums, value_variant, 0).unwrap();
    let identity = test_slot_identity(&value);
    let slot = CoroutineSlot::checked(&module.enums, value_payload, empty, value, identity)
        .expect("valid CoroutineSlot metadata");
    let slot_id = module.meta.coroutine_slots.alloc(slot);
    assert_eq!(module.validate(), Ok(()));

    module.enums[enum_id].variants.pop();
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::CoroutineSlot { slot: slot_id },
            kind: MirValidationErrorKind::InvalidCoroutineSlot,
        })
    );
}
