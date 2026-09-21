use super::*;

#[test]
fn independent_source_inventory_and_complete_records_produce_a_local_join() {
    let source = Source::new(ConeIdentity::SINGLE_FILE);
    let exports = source.exports();
    let proof = exports
        .validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(proof.provider(), source.provider);
    assert!(std::ptr::eq(proof.exports(), &exports));
    assert_eq!(proof.exports().types().records().len(), 4);
}

#[test]
fn source_inventory_is_not_inferred_from_the_transport_table() {
    let mut source = Source::new(ConeIdentity::SINGLE_FILE);
    let exports = source.exports();
    source.required_types.pop();
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::Inventory(
            MirTypeBridgeSourceInventoryV1::Types
        ))
    ));
    source.required_types.push(source.required_types[0]);
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::NonCanonicalInventory(
            MirTypeBridgeSourceInventoryV1::Types
        ))
    ));
}

#[test]
fn every_export_inventory_is_checked_even_when_the_transport_table_is_empty() {
    let mut source = Source::new(ConeIdentity::SINGLE_FILE);
    let exports = source.exports();
    source
        .required_callables
        .push(StrongCallableDefinitionOwner::Function(
            source.frame_owner(),
        ));
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::Inventory(
            MirTypeBridgeSourceInventoryV1::Callables
        ))
    ));
    source.required_callables.clear();
    source.required_dispatch.push(source.fixture.payload.id());
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::Inventory(
            MirTypeBridgeSourceInventoryV1::Dispatch
        ))
    ));
    source.required_dispatch.clear();
    let value: scoop_identity::CborIdentityRecord<PersistentObjectValueId, _> =
        scoop_identity::CborIdentityRecord::from_key(source.fixture.object.key().clone()).unwrap();
    source.required_objects.push(value.id());
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::Inventory(
            MirTypeBridgeSourceInventoryV1::Objects
        ))
    ));
}

#[test]
fn same_exact_with_changed_fields_and_facts_cannot_replace_the_source_record() {
    let source = Source::new(ConeIdentity::SINGLE_FILE);
    let mut exports = source.exports();
    let replacement = source
        .fixture
        .source(
            source.fixture.empty.id(),
            MirTypeFactsV1::try_new(MirValueKindV1::NonZeroValue, MirGcKindV1::GcFree).unwrap(),
            MirTypeRepresentationV1::Struct {
                fields: vec![MirRepresentationFieldV1 {
                    field: source.fixture.fields[0].id(),
                    value: source.fixture.payload.id(),
                }],
                c_layout: MirTypeCLayoutPolicyV1::Ordinary,
                interior_mutable: false,
            },
        )
        .unwrap();
    exports.types = CanonicalParamFreeMirTypeExportsV1::try_new(
        exports
            .types
            .records()
            .iter()
            .map(|record| {
                if record.exact() == replacement.exact() {
                    replacement.clone()
                } else {
                    record.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(
        matches!(exports.validate_sources(source.provider, &source.fixture.graph, &source, &mut meter()), Err(MirTypeBridgeSourceJoinError::Record(MirTypeBridgeSourceRecordV1::Type(exact))) if exact == source.fixture.payload.id())
    );
}

#[test]
fn local_source_join_does_not_accept_a_different_manifest_provider() {
    let source = Source::new(ConeIdentity::SINGLE_FILE);
    assert!(matches!(
        source.exports().validate_sources(
            ConeIdentity::CORE,
            &source.fixture.graph,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::Provider)
    ));
}

#[test]
fn source_comparisons_share_an_inclusive_validation_budget() {
    let source = Source::new(ConeIdentity::SINGLE_FILE);
    let exports = source.exports();
    let mut measured = meter();
    exports
        .validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut measured,
        )
        .unwrap();
    let work = measured.usage().validation_work_units;
    exports
        .validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: work,
                ..DecodeLimits::default()
            }),
        )
        .unwrap();
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            &source,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: work - 1,
                ..DecodeLimits::default()
            })
        ),
        Err(MirTypeBridgeSourceJoinError::Resource(_))
            | Err(MirTypeBridgeSourceJoinError::Shape(
                MirShapeSupportError::Resource(_)
            ))
    ));
}
