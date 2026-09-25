use super::*;

#[test]
fn complete_six_domain_body_matches_exact_receiver_and_occurrence_sets() {
    let f = Fixture::new();
    let owner = SignatureTypeKey::Nominal(f.type_id);
    let constructor = DefaultConstructorRefV1::Struct {
        declaration: f.constructor,
        owner_type: owner.clone(),
    };
    let field = DefaultFieldRefV1::Struct {
        declaration: f.field,
        owner_type: owner.clone(),
    };
    let elements = vec![
        expression(
            &f,
            DefaultExpressionKindV1::Call {
                receiver: crate::SourceCallReceiver::NoReceiver,
                callee: f.callable(),
                arguments: vec![],
            },
        ),
        expression(
            &f,
            DefaultExpressionKindV1::StructInit {
                constructor: constructor.clone(),
                arguments: vec![],
            },
        ),
        expression(&f, DefaultExpressionKindV1::GlobalRead(f.property)),
        expression(&f, DefaultExpressionKindV1::SingletonValue(f.object)),
        expression(
            &f,
            DefaultExpressionKindV1::FieldAccess {
                receiver: Box::new(expression(
                    &f,
                    DefaultExpressionKindV1::GlobalRead(f.property),
                )),
                field: field.clone(),
            },
        ),
    ];
    let no_receiver = |index| use_at(index, ProtectedDefaultReceiverUseV1::None);
    let refs = ProtectedDefaultReferenceSetV1::try_new(
        vec![reference(
            &f,
            ExportDefaultCallableTargetV1::Callable(f.callable()),
            vec![no_receiver(1)],
        )],
        vec![reference(&f, constructor, vec![no_receiver(2)])],
        vec![reference(&f, owner, vec![no_receiver(2), no_receiver(5)])],
        vec![reference(
            &f,
            f.property,
            vec![no_receiver(3), no_receiver(6)],
        )],
        vec![reference(&f, f.object, vec![no_receiver(4)])],
        vec![reference(
            &f,
            field,
            vec![use_at(
                5,
                ProtectedDefaultReceiverUseV1::Explicit {
                    receiver_expression_index: 6,
                },
            )],
        )],
    )
    .unwrap();
    let input = Input {
        body: body(&f, DefaultExpressionKindV1::TupleLiteral(elements)),
        f,
        refs,
        locals: empty_locals(),
        receiver: OptionalTemplateReceiverV1::Absent,
    };
    let mut authority = Authority::default();
    input.validate(&mut authority, &mut meter()).unwrap();
    assert_eq!(authority.members, vec![(false, false)]);
}

#[test]
fn repeated_target_at_different_origins_keeps_separate_records_and_uses() {
    let f = Fixture::new();
    let source = f.origin().origin().source().clone();
    let context = scoop_identity::SourceContextKey::File {
        source: source.clone(),
    };
    let other = ExportDefinitionSourceV1::new(
        scoop_identity::DefinitionOrigin::new(
            source,
            scoop_identity::SourceSpan::new(20, 21).unwrap(),
            &context,
        )
        .unwrap(),
    );
    let first = expression(&f, DefaultExpressionKindV1::GlobalRead(f.property));
    let second = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::GlobalRead(f.property),
        f.value_type(),
        other.clone(),
    )
    .unwrap();
    let first_record = reference(
        &f,
        f.property,
        vec![use_at(1, ProtectedDefaultReceiverUseV1::None)],
    );
    let second_record = ProtectedDefaultReferenceV1::new(
        f.property,
        other,
        first_record.witness().clone(),
        CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![use_at(
            2,
            ProtectedDefaultReceiverUseV1::None,
        )])
        .unwrap(),
    );
    let refs = ProtectedDefaultReferenceSetV1::try_new(
        vec![],
        vec![],
        vec![],
        vec![second_record, first_record],
        vec![],
        vec![],
    )
    .unwrap();
    let input = Input {
        body: body(
            &f,
            DefaultExpressionKindV1::TupleLiteral(vec![first, second]),
        ),
        f,
        refs,
        locals: empty_locals(),
        receiver: OptionalTemplateReceiverV1::Absent,
    };
    input
        .validate(&mut Authority::default(), &mut meter())
        .unwrap();
}
