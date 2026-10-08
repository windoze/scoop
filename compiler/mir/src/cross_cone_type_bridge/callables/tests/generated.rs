use super::*;

#[test]
fn adjusts_keep_the_exact_target_and_receiver_conversion() {
    let fixture = Fixture::new();
    let value_signature = sig(Some(fixture.value), vec![], fixture.unit);
    let boxed_signature = sig(Some(fixture.boxed), vec![], fixture.unit);
    let value_target = StrongCallableDefinitionOwner::Function(fixture.method.id()).into();
    ParamFreeMirCallableBindingV1::try_new(
        fixture.authority(),
        generated(&fixture.boxing),
        StrongCallableDefinitionOwner::GeneratedCallable(fixture.boxing.id()),
        MirBridgeCallableSignatureV1::new(value_signature.clone(), crate::GcEffect::NoGc),
        signature(boxed_signature.clone()),
        MirCallableLoweringRoleV1::BoxingAdjust {
            target: value_target,
        },
    )
    .unwrap();
    for (identity, semantic, lowered, role) in [
        (
            &fixture.adjust,
            sig(Some(fixture.interface), vec![], fixture.unit),
            sig(Some(fixture.class), vec![], fixture.unit),
            MirCallableLoweringRoleV1::DispatchAdjust {
                target: StrongCallableDefinitionOwner::Function(fixture.abstract_method.id())
                    .into(),
            },
        ),
        (
            &fixture.boxing,
            value_signature,
            boxed_signature,
            MirCallableLoweringRoleV1::BoxingAdjust {
                target: value_target,
            },
        ),
    ] {
        fixture
            .bind(generated(identity), semantic.clone(), lowered, role)
            .unwrap();
        assert!(matches!(
            fixture.bind(
                generated(identity),
                semantic,
                sig(Some(fixture.interface), vec![], fixture.unit),
                role
            ),
            Err(MirCallableBridgeError::FoundationSignatureMismatch { .. })
        ));
    }
    assert!(matches!(
        fixture.bind(
            generated(&fixture.adjust),
            sig(Some(fixture.interface), vec![], fixture.unit),
            sig(Some(fixture.class), vec![], fixture.unit),
            MirCallableLoweringRoleV1::DispatchAdjust {
                target: value_target
            }
        ),
        Err(MirCallableBridgeError::InvalidAdjustTarget)
    ));
    assert!(matches!(
        fixture.bind(
            generated(&fixture.adjust),
            sig(Some(fixture.value), vec![], fixture.unit),
            sig(Some(fixture.class), vec![], fixture.unit),
            MirCallableLoweringRoleV1::BoxingAdjust {
                target: value_target
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
