use super::*;

#[test]
fn adjusts_keep_the_exact_target_and_receiver_conversion() {
    let fixture = Fixture::new();
    let semantic = sig(Some(fixture.value), vec![], fixture.unit);
    let lowered = sig(Some(fixture.interface), vec![], fixture.unit);
    let target = StrongCallableDefinitionOwner::Function(fixture.method.id());
    ParamFreeMirCallableBindingV1::try_new(
        fixture.authority(),
        generated(&fixture.boxing),
        StrongCallableDefinitionOwner::GeneratedCallable(fixture.boxing.id()),
        MirBridgeCallableSignatureV1::new(semantic.clone(), crate::GcEffect::NoGc),
        signature(lowered.clone()),
        MirCallableLoweringRoleV1::BoxingAdjust {
            target: scoop_identity::CallableDefinitionOwner::Strong(target),
        },
    )
    .unwrap();
    for (identity, role) in [
        (
            &fixture.adjust,
            MirCallableLoweringRoleV1::DispatchAdjust {
                target: scoop_identity::CallableDefinitionOwner::Strong(target),
            },
        ),
        (
            &fixture.boxing,
            MirCallableLoweringRoleV1::BoxingAdjust {
                target: scoop_identity::CallableDefinitionOwner::Strong(target),
            },
        ),
    ] {
        fixture
            .bind(generated(identity), semantic.clone(), lowered.clone(), role)
            .unwrap();
        assert!(matches!(
            fixture.bind(
                generated(identity),
                sig(Some(fixture.class), vec![], fixture.unit),
                lowered.clone(),
                role
            ),
            Err(MirCallableBridgeError::InvalidAdjustTarget)
        ));
    }
    assert!(matches!(
        fixture.bind(
            generated(&fixture.adjust),
            semantic.clone(),
            lowered.clone(),
            MirCallableLoweringRoleV1::DispatchAdjust {
                target: scoop_identity::CallableDefinitionOwner::Strong(
                    StrongCallableDefinitionOwner::Function(fixture.abstract_method.id())
                )
            }
        ),
        Err(MirCallableBridgeError::InvalidAdjustTarget)
    ));
    assert!(matches!(
        fixture.bind(
            generated(&fixture.adjust),
            semantic,
            lowered,
            MirCallableLoweringRoleV1::BoxingAdjust {
                target: scoop_identity::CallableDefinitionOwner::Strong(target)
            }
        ),
        Err(MirCallableBridgeError::RoleMismatch)
    ));
}

#[test]
fn object_ensure_and_initializer_share_unit_but_not_generated_role() {
    let fixture = Fixture::new();
    let GeneratedCallableKey::Initialization { unit, .. } = fixture.ensure.key() else {
        unreachable!()
    };
    let signature = sig(None, vec![], fixture.unit);
    for (identity, role) in [
        (
            &fixture.ensure,
            MirCallableLoweringRoleV1::ObjectEnsure { unit: *unit },
        ),
        (
            &fixture.initializer,
            MirCallableLoweringRoleV1::ObjectInitializer { unit: *unit },
        ),
    ] {
        fixture
            .bind(
                generated(identity),
                signature.clone(),
                signature.clone(),
                role,
            )
            .unwrap();
    }
    assert!(matches!(
        fixture.bind(
            generated(&fixture.ensure),
            signature.clone(),
            signature,
            MirCallableLoweringRoleV1::ObjectInitializer { unit: *unit }
        ),
        Err(MirCallableBridgeError::RoleMismatch)
    ));
    let no_gc =
        MirBridgeCallableSignatureV1::new(sig(None, vec![], fixture.unit), crate::GcEffect::NoGc);
    assert!(matches!(
        ParamFreeMirCallableBindingV1::try_new(
            fixture.authority(),
            generated(&fixture.ensure),
            StrongCallableDefinitionOwner::GeneratedCallable(fixture.ensure.id()),
            no_gc.clone(),
            no_gc,
            MirCallableLoweringRoleV1::ObjectEnsure { unit: *unit }
        ),
        Err(MirCallableBridgeError::SignatureMismatch)
    ));
}

#[test]
fn derived_equality_uses_value_receiver_and_exact_boolean_result() {
    let fixture = Fixture::new();
    let signature = sig(Some(fixture.value), vec![fixture.value], fixture.boolean);
    fixture
        .bind(
            generated(&fixture.equality),
            signature.clone(),
            signature,
            MirCallableLoweringRoleV1::DerivedEquality {
                owner: fixture.value,
            },
        )
        .unwrap();
}

#[test]
fn generated_origin_role_must_equal_its_canonical_identity() {
    let fixture = Fixture::new();
    let signature = sig(None, vec![], fixture.unit);
    let GeneratedCallableKey::Initialization { unit, .. } = fixture.ensure.key() else {
        unreachable!()
    };
    assert!(matches!(
        fixture.bind(
            MirCallableOriginV1::Generated {
                callable: fixture.ensure.id(),
                role: fixture.initializer.key().clone()
            },
            signature.clone(),
            signature,
            MirCallableLoweringRoleV1::ObjectEnsure { unit: *unit }
        ),
        Err(MirCallableBridgeError::GeneratedRoleMismatch)
    ));
}
