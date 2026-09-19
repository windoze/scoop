use super::*;

fn member_input(kind: u8, implicit: bool) -> Input {
    let f = Fixture::new();
    let owner = SignatureTypeKey::Nominal(f.type_id);
    let receiver = DefaultExpressionV1::try_new(
        if implicit {
            DefaultExpressionKindV1::Local(LocalValueSelector::This)
        } else {
            DefaultExpressionKindV1::GlobalRead(f.property)
        },
        owner.clone(),
        f.origin(),
    )
    .unwrap();
    let field = DefaultFieldRefV1::Struct {
        declaration: f.field,
        owner_type: owner.clone(),
    };
    let callable = DefaultCallableRefV1::try_new(
        DefaultCallableDeclarationV1::Function(f.function),
        scoop_identity::OptionalSignatureType::Present(Box::new(owner.clone())),
        vec![],
    )
    .unwrap();
    let callee = DefaultMethodCalleeV1::Callable(callable.clone());
    let value = match kind {
        0 => DefaultExpressionKindV1::FieldAccess {
            receiver: Box::new(receiver),
            field: field.clone(),
        },
        1 => DefaultExpressionKindV1::MethodCall {
            receiver: Box::new(receiver),
            callee,
            arguments: vec![],
        },
        2 => DefaultExpressionKindV1::DirectSuperMethodCall {
            receiver: Box::new(receiver),
            callee,
            arguments: vec![],
        },
        _ => panic!("test member kind"),
    };
    let mut refs = empty_set();
    let usage = use_at(
        0,
        if implicit {
            ProtectedDefaultReceiverUseV1::ImplicitThis
        } else {
            ProtectedDefaultReceiverUseV1::Explicit {
                receiver_expression_index: 1,
            }
        },
    );
    if kind == 0 {
        refs.fields = vec![reference(&f, field, vec![usage])];
    } else {
        refs.callables = vec![reference(
            &f,
            ExportDefaultCallableTargetV1::Callable(callable),
            vec![usage],
        )];
    }
    refs.types = vec![reference(
        &f,
        owner.clone(),
        vec![
            use_at(0, ProtectedDefaultReceiverUseV1::None),
            use_at(1, ProtectedDefaultReceiverUseV1::None),
        ],
    )];
    let (receiver, locals) = if implicit {
        (
            OptionalTemplateReceiverV1::Present(
                TemplateReceiverV1::try_new(LocalValueSelector::This, owner.clone()).unwrap(),
            ),
            CanonicalTemplateLocalTableV1::try_new(vec![
                TemplateLocalRecordV1::try_new(
                    LocalValueSelector::This,
                    owner,
                    CanonicalBooleanV1::False,
                    TemplateLocalDefinitionV1::Source(f.origin()),
                )
                .unwrap(),
            ])
            .unwrap(),
        )
    } else {
        refs.globals = vec![reference(
            &f,
            f.property,
            vec![use_at(1, ProtectedDefaultReceiverUseV1::None)],
        )];
        (OptionalTemplateReceiverV1::Absent, empty_locals())
    };
    Input {
        body: body(&f, value),
        f,
        receiver,
        locals,
        refs,
    }
}

#[test]
fn members_use_actual_receiver_and_preserve_direct_super_context() {
    for kind in 0..=2 {
        for implicit in [false, true] {
            let input = member_input(kind, implicit);
            let mut authority = Authority::default();
            input.validate(&mut authority, &mut meter()).unwrap();
            assert_eq!(authority.members, vec![(implicit, kind == 2)]);
        }
    }
}

#[test]
fn receiver_use_cannot_claim_another_expression_or_fake_implicit_this() {
    let mut input = member_input(0, false);
    for receiver in [
        ProtectedDefaultReceiverUseV1::None,
        ProtectedDefaultReceiverUseV1::ImplicitThis,
        ProtectedDefaultReceiverUseV1::Explicit {
            receiver_expression_index: 0,
        },
        ProtectedDefaultReceiverUseV1::Explicit {
            receiver_expression_index: u32::MAX,
        },
        ProtectedDefaultReceiverUseV1::ConstructorDelegation,
    ] {
        let target = input.refs.fields[0].target().clone();
        input.refs.fields[0] = reference(&input.f, target, vec![use_at(0, receiver)]);
        assert!(matches!(
            input.validate(&mut Authority::default(), &mut meter()),
            Err(ProtectedDefaultBodyClosureError::MissingUse { .. })
        ));
    }
}

#[test]
fn field_assignment_metadata_keeps_its_actual_receiver_and_empty_uses() {
    let f = Fixture::new();
    let nominal = SignatureTypeKey::Nominal(f.type_id);
    let field = DefaultFieldRefV1::Struct {
        declaration: f.field,
        owner_type: nominal.clone(),
    };
    let statement = DefaultStatementV1::try_new(
        DefaultStatementKindV1::Assign {
            target: Box::new(DefaultAssignTargetV1::Field {
                receiver: Box::new(expression(
                    &f,
                    DefaultExpressionKindV1::GlobalRead(f.property),
                )),
                field: field.clone(),
            }),
            value: Box::new(expression(&f, DefaultExpressionKindV1::UnitLiteral)),
        },
        f.origin(),
    )
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        vec![statement],
        expression(&f, DefaultExpressionKindV1::UnitLiteral),
    )
    .unwrap();
    let mut refs = empty_set();
    refs.fields = vec![reference(&f, field, vec![])];
    refs.types = vec![reference(&f, nominal, vec![])];
    refs.globals = vec![reference(
        &f,
        f.property,
        vec![use_at(0, ProtectedDefaultReceiverUseV1::None)],
    )];
    let input = Input {
        f,
        body,
        locals: empty_locals(),
        receiver: OptionalTemplateReceiverV1::Absent,
        refs,
    };
    let mut authority = Authority::default();
    input.validate(&mut authority, &mut meter()).unwrap();
    assert_eq!(authority.assignments, 2); // Field and its owner-type metadata.
    assert!(matches!(
        input.validate(
            &mut Authority {
                reject_metadata: true,
                ..Authority::default()
            },
            &mut meter()
        ),
        Err(ProtectedDefaultBodyClosureError::Source(_))
    ));
}
