use super::*;

#[test]
fn class_initializer_has_a_separate_receiver_and_unit_result() {
    let fixture = Fixture::new();
    let origin = MirCallableOriginV1::Constructor(fixture.constructors[0].id());
    let semantic = sig(None, vec![], fixture.class);
    let lowered = sig(Some(fixture.class), vec![], fixture.unit);
    let role = MirCallableLoweringRoleV1::ClassInitializer {
        owner: fixture.class,
    };
    let binding = fixture
        .bind(origin.clone(), semantic.clone(), lowered.clone(), role)
        .unwrap();
    let suspended = MirBridgeCallableSignatureV1::new(
        ExactCallableSignature::new(Effect::Suspend, None, vec![], fixture.class),
        crate::GcEffect::Managed,
    );
    assert!(matches!(
        ParamFreeMirCallableBindingV1::try_new(
            fixture.authority(),
            origin.clone(),
            origin.implementation().unwrap(),
            suspended,
            binding.lowered_signature().clone(),
            role
        ),
        Err(MirCallableBridgeError::SuspendSynchronousRole)
    ));
    assert_ne!(binding.semantic_signature(), binding.lowered_signature());
    assert!(matches!(
        fixture.bind(origin.clone(), semantic.clone(), semantic, role),
        Err(MirCallableBridgeError::FoundationSignatureMismatch { .. })
    ));
    assert!(matches!(
        fixture.bind(
            origin.clone(),
            sig(None, vec![], fixture.value),
            lowered.clone(),
            role
        ),
        Err(MirCallableBridgeError::SignatureMismatch)
    ));
    assert!(matches!(
        fixture.bind(
            origin,
            sig(None, vec![], fixture.class),
            lowered,
            MirCallableLoweringRoleV1::ClassInitializer {
                owner: fixture.value
            }
        ),
        Err(MirCallableBridgeError::ConstructorOwnerMismatch)
    ));
}

#[test]
fn value_constructor_is_by_value_and_accessor_roles_keep_their_shapes() {
    let fixture = Fixture::new();
    let signature = sig(None, vec![], fixture.value);
    fixture
        .bind(
            MirCallableOriginV1::Constructor(fixture.constructors[1].id()),
            signature.clone(),
            signature,
            MirCallableLoweringRoleV1::ValueConstructor {
                owner: fixture.value,
            },
        )
        .unwrap();
    for (accessor, signature) in fixture.accessors.iter().zip([
        sig(Some(fixture.class), vec![], fixture.value),
        sig(Some(fixture.class), vec![fixture.value], fixture.unit),
    ]) {
        fixture
            .bind(
                MirCallableOriginV1::Accessor(accessor.id()),
                signature.clone(),
                signature.clone(),
                MirCallableLoweringRoleV1::Accessor,
            )
            .unwrap();
        assert!(matches!(
            fixture.bind(
                MirCallableOriginV1::Accessor(accessor.id()),
                signature.clone(),
                signature,
                MirCallableLoweringRoleV1::Ordinary
            ),
            Err(MirCallableBridgeError::RoleMismatch)
        ));
    }
}

#[test]
fn callable_identity_and_gc_contract_cannot_be_relabelled() {
    let fixture = Fixture::new();
    let binding = fixture.method_binding();
    assert!(matches!(
        ParamFreeMirCallableBindingV1::try_new(
            fixture.authority(),
            binding.origin().clone(),
            StrongCallableDefinitionOwner::Function(fixture.abstract_method.id()),
            binding.semantic_signature().clone(),
            binding.lowered_signature().clone(),
            *binding.lowering_role()
        ),
        Err(MirCallableBridgeError::OriginMismatch)
    ));
    let no_gc = MirBridgeCallableSignatureV1::new(
        binding.lowered_signature().exact().clone(),
        crate::GcEffect::NoGc,
    );
    assert!(matches!(
        ParamFreeMirCallableBindingV1::try_new(
            fixture.authority(),
            binding.origin().clone(),
            binding.implementation(),
            binding.semantic_signature().clone(),
            no_gc,
            *binding.lowering_role()
        ),
        Err(MirCallableBridgeError::SignatureMismatch)
    ));
    let reference_signature = MirBridgeCallableSignatureV1::new(
        sig(Some(fixture.interface), vec![], fixture.unit),
        crate::GcEffect::NoGc,
    );
    assert!(matches!(
        ParamFreeMirCallableBindingV1::try_new(
            fixture.authority(),
            MirCallableOriginV1::Function(fixture.abstract_method.id()),
            StrongCallableDefinitionOwner::Function(fixture.abstract_method.id()),
            reference_signature.clone(),
            reference_signature,
            MirCallableLoweringRoleV1::PureVirtualTrap {
                slot: fixture.slot.id()
            }
        ),
        Err(MirCallableBridgeError::NoGcContainsReferences { .. })
    ));
    let suspend = MirBridgeCallableSignatureV1::new(
        ExactCallableSignature::new(Effect::Suspend, Some(fixture.value), vec![], fixture.unit),
        crate::GcEffect::Managed,
    );
    assert!(matches!(
        ParamFreeMirCallableBindingV1::try_new(
            fixture.authority(),
            binding.origin().clone(),
            binding.implementation(),
            suspend,
            binding.lowered_signature().clone(),
            *binding.lowering_role()
        ),
        Err(MirCallableBridgeError::SignatureMismatch)
    ));
}

#[test]
fn abstract_trap_target_is_the_original_slot_declaration() {
    let fixture = Fixture::new();
    let signature = sig(Some(fixture.interface), vec![], fixture.unit);
    fixture
        .bind(
            MirCallableOriginV1::Function(fixture.abstract_method.id()),
            signature.clone(),
            signature,
            MirCallableLoweringRoleV1::PureVirtualTrap {
                slot: fixture.slot.id(),
            },
        )
        .unwrap();
    let method = fixture.method_binding();
    assert!(matches!(
        fixture.bind(
            method.origin().clone(),
            method.semantic_signature().exact().clone(),
            method.lowered_signature().exact().clone(),
            MirCallableLoweringRoleV1::PureVirtualTrap {
                slot: fixture.slot.id()
            }
        ),
        Err(MirCallableBridgeError::InvalidTrapDeclaration)
    ));
}
