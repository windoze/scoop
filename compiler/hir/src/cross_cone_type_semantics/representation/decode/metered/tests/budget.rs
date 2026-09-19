use super::*;

#[test]
fn inline_resource_failures_precede_every_representation_resolver_call() {
    let (mut fixture, record) = fixtures::structure(unit(), 2);
    let decoded: DecodedNominalRepresentationSupportV1 = parsed(&record);
    let origin = path().field(2).field(3);
    let cases = [
        (
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedNodes,
            path(),
        ),
        (
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedEdges,
            path(),
        ),
        (
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
            path(),
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
            path(),
        ),
        (
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
            origin.clone(),
        ),
        (
            DecodeLimits {
                semantic_recursion: 4,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticRecursion,
            origin.clone(),
        ),
        (
            DecodeLimits {
                semantic_leaf_bytes: 10,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticLeafBytes,
            origin,
        ),
        (
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticTableEntries,
            path().field(3).field(1),
        ),
    ];
    for (limits, kind, at) in cases {
        let mut resolver = Counting::new(&mut fixture);
        let error = decoded
            .clone()
            .resolve_metered(&mut resolver, &mut BudgetMeter::new(limits), &path())
            .unwrap_err();
        resource(error, kind, &at);
        assert!(resolver.calls.is_empty());
    }
    // The actual path is eleven bytes; its enclosing origin product is larger.
    let mut resources = BudgetMeter::new(DecodeLimits {
        semantic_leaf_bytes: 11,
        ..DecodeLimits::default()
    });
    assert_eq!(
        decoded
            .resolve_metered(&mut Counting::new(&mut fixture), &mut resources, &path())
            .unwrap(),
        record
    );
}

#[test]
fn deep_and_wide_signatures_fail_at_the_actual_field_before_resolution() {
    let mut deep = unit();
    for _ in 0..32 {
        deep = SignatureTypeKey::RawPointer(Box::new(deep));
    }
    let wide = SignatureTypeKey::Tuple(NonEmptyVec::new(vec![unit(); 4096]).unwrap());
    let at = path().field(3).field(1).index(0).field(2);
    for (signature, limits, kind) in [
        (
            deep,
            DecodeLimits {
                semantic_recursion: 16,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticRecursion,
        ),
        (
            wide.clone(),
            DecodeLimits {
                semantic_table_entries: 128,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticTableEntries,
        ),
        (
            wide,
            DecodeLimits {
                validation_work_units: 2048,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
        ),
    ] {
        let (mut fixture, record) = fixtures::structure(signature, 1);
        let decoded: DecodedNominalRepresentationSupportV1 = parsed(&record);
        let mut resolver = Counting::new(&mut fixture);
        resource(
            decoded
                .resolve_metered(&mut resolver, &mut BudgetMeter::new(limits), &path())
                .unwrap_err(),
            kind,
            &at,
        );
        assert!(resolver.calls.is_empty());
    }
}

#[test]
fn returned_source_and_field_names_are_checked_before_hashing_or_type_resolution() {
    let (mut fixture, record) = fixtures::structure(unit(), 1);
    let decoded: DecodedNominalRepresentationSupportV1 = parsed(&record);
    let long = "x".repeat(128);
    let source = source_key(&long, SourceNominalKind::Struct);
    let field =
        FieldIdentityKey::source_declared(&fixture.key, CanonicalIdentifier::new(&long).unwrap())
            .unwrap();
    let limits = DecodeLimits {
        semantic_leaf_bytes: 64,
        ..DecodeLimits::default()
    };
    let mut resolver = Counting::new(&mut fixture);
    resolver.source_override = Some(Arc::new(source));
    resource(
        decoded
            .clone()
            .resolve_metered(&mut resolver, &mut BudgetMeter::new(limits), &path())
            .unwrap_err(),
        ResourceKind::SemanticLeafBytes,
        &path().field(1),
    );
    assert_eq!(resolver.calls, ["source key"]);
    resolver.calls.clear();
    resolver.source_override = None;
    resolver.field_override = Some(Arc::new(field));
    resource(
        decoded
            .resolve_metered(&mut resolver, &mut BudgetMeter::new(limits), &path())
            .unwrap_err(),
        ResourceKind::SemanticLeafBytes,
        &path().field(3).field(1).index(0).field(1),
    );
    assert_eq!(resolver.calls.last(), Some(&"field"));
    assert!(!resolver.calls.contains(&"type"));
}

#[test]
fn representation_resolution_uses_the_callers_accumulated_meter() {
    let (mut fixture, record) = fixtures::structure(unit(), 1);
    let decoded: DecodedNominalRepresentationSupportV1 = parsed(&record);
    let mut baseline = meter();
    decoded
        .clone()
        .resolve_metered(&mut Counting::new(&mut fixture), &mut baseline, &path())
        .unwrap();
    let limit = baseline.usage().validation_work_units * 2 - 1;
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: limit,
        ..DecodeLimits::default()
    });
    let mut resolver = Counting::new(&mut fixture);
    decoded
        .clone()
        .resolve_metered(&mut resolver, &mut shared, &path())
        .unwrap();
    let first = shared.usage();
    let error = decoded
        .resolve_metered(&mut resolver, &mut shared, &path())
        .unwrap_err();
    let MeteredNominalRepresentationResolutionError::Resource(error) = error else {
        panic!("{error:?}");
    };
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::ValidationWorkUnits,
            ..
        }
    ));
    assert!(shared.usage().validation_work_units >= first.validation_work_units);
    assert!(shared.usage().owned_bytes > first.owned_bytes);
}
