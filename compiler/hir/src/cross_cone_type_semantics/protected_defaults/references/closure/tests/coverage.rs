use super::*;

#[test]
fn metadata_only_records_require_full_source_replay_and_no_synthetic_use() {
    let mut input = metadata_input();
    let mut authority = Authority::default();
    input.validate(&mut authority, &mut meter()).unwrap();
    assert_eq!(authority.metadata, 1);
    assert!(matches!(
        input.validate(
            &mut Authority {
                reject_metadata: true,
                ..Authority::default()
            },
            &mut meter()
        ),
        Err(ProtectedDefaultBodyClosureError::Source(
            "definition-side metadata access rejected"
        ))
    ));
    input.refs.types[0] = reference(
        &input.f,
        SignatureTypeKey::Nominal(input.f.type_id),
        vec![use_at(0, ProtectedDefaultReceiverUseV1::None)],
    );
    assert!(matches!(
        input.validate(&mut Authority::default(), &mut meter()),
        Err(ProtectedDefaultBodyClosureError::ExtraUse {
            kind: ProtectedDefaultReferenceKindV1::Type,
            ..
        })
    ));
}

#[test]
fn same_record_collects_metadata_and_expression_uses_without_skipping_either() {
    let mut input = metadata_input();
    let nominal = SignatureTypeKey::Nominal(input.f.type_id);
    input.body = ExportDefaultBodyV1::try_new(
        vec![],
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::Local(input.f.local()),
            nominal.clone(),
            input.f.origin(),
        )
        .unwrap(),
    )
    .unwrap();
    input.refs.types[0] = reference(
        &input.f,
        nominal,
        vec![use_at(0, ProtectedDefaultReceiverUseV1::None)],
    );
    let mut authority = Authority::default();
    input.validate(&mut authority, &mut meter()).unwrap();
    assert_eq!(authority.metadata, 1);
}

#[test]
fn exact_reference_closure_rejects_missing_extra_wrong_origin_and_owner() {
    let mut input = metadata_input();
    let original = input.refs.clone();
    input.refs = empty_set();
    assert!(matches!(
        input.validate(&mut Authority::default(), &mut meter()),
        Err(ProtectedDefaultBodyClosureError::Missing { .. })
    ));
    input.refs = original.clone();
    input
        .refs
        .globals
        .push(reference(&input.f, input.f.property, vec![]));
    assert!(matches!(
        input.validate(&mut Authority::default(), &mut meter()),
        Err(ProtectedDefaultBodyClosureError::Extra { .. })
    ));
    input.refs = original.clone();
    let record = &input.refs.types[0];
    let source = record.definition_origin().origin().source().clone();
    let context = scoop_identity::SourceContextKey::File {
        source: source.clone(),
    };
    let origin = ExportDefinitionSourceV1::new(
        scoop_identity::DefinitionOrigin::new(
            source,
            scoop_identity::SourceSpan::new(20, 21).unwrap(),
            &context,
        )
        .unwrap(),
    );
    input.refs.types[0] = ProtectedDefaultReferenceV1::new(
        record.target().clone(),
        origin,
        record.witness().clone(),
        record.uses().clone(),
    );
    assert!(matches!(
        input.validate(&mut Authority::default(), &mut meter()),
        Err(ProtectedDefaultBodyClosureError::Missing { .. })
    ));
    input.refs = original;
    let record = &input.refs.types[0];
    input.refs.types[0] = ProtectedDefaultReferenceV1::new(
        record.target().clone(),
        record.definition_origin().clone(),
        ProtectedDefaultAccessWitnessV1::generic_source_metadata(
            CallableTemplateOrigin::Constructor(input.f.constructor),
        )
        .unwrap(),
        record.uses().clone(),
    );
    assert!(matches!(
        input.validate(&mut Authority::default(), &mut meter()),
        Err(ProtectedDefaultBodyClosureError::WitnessOwner { .. })
    ));
}

#[test]
fn repeated_body_target_requires_each_distinct_expression_index() {
    let f = Fixture::new();
    let mut refs = empty_set();
    refs.globals = vec![reference(
        &f,
        f.property,
        vec![
            use_at(1, ProtectedDefaultReceiverUseV1::None),
            use_at(2, ProtectedDefaultReceiverUseV1::None),
        ],
    )];
    let mut input = Input {
        body: body(
            &f,
            DefaultExpressionKindV1::TupleLiteral(vec![
                expression(&f, DefaultExpressionKindV1::GlobalRead(f.property)),
                expression(&f, DefaultExpressionKindV1::GlobalRead(f.property)),
            ]),
        ),
        f,
        locals: empty_locals(),
        receiver: OptionalTemplateReceiverV1::Absent,
        refs,
    };
    input
        .validate(&mut Authority::default(), &mut meter())
        .unwrap();
    input.refs.globals[0] = reference(
        &input.f,
        input.f.property,
        vec![use_at(1, ProtectedDefaultReceiverUseV1::None)],
    );
    assert!(
        matches!(input.validate(&mut Authority::default(), &mut meter()), Err(ProtectedDefaultBodyClosureError::MissingUse { expected, .. }) if expected.expression_index() == 2)
    );
}

#[test]
fn body_collection_and_matching_share_resource_limits() {
    let input = metadata_input();
    for limits in [
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            input.validate(&mut Authority::default(), &mut BudgetMeter::new(limits)),
            Err(ProtectedDefaultBodyClosureError::Resource(_))
        ));
    }
}
