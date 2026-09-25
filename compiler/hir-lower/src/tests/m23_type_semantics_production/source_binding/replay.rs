use super::*;

#[test]
fn byte_bound_sources_replay_gc_zst_inheritance_and_object_relations() {
    let output = lower_public_declarations(vec![
        struct_decl("EmptyValue", vec![]),
        enum_decl(
            "Flag",
            vec![],
            vec![variant_unit("Off"), variant_unit("On")],
        ),
        class_decl(
            ast::ClassModifier::Open,
            "Base",
            vec![],
            None,
            vec![],
            vec![],
        ),
        interface_decl("Face", vec![]),
        class_decl(
            ast::ClassModifier::Final,
            "Derived",
            vec![],
            Some(("Base", vec![])),
            vec!["Face"],
            vec![],
        ),
        object_decl("Singleton"),
    ]);
    let production =
        produce_cross_cone_type_semantics(&output, &public_interface(&output)).unwrap();
    let fixture = Fixture::from_output(&output);
    assert!(
        fixture
            .source
            .entries()
            .dependency_facts
            .records()
            .is_empty()
    );
    let bound = fixture.bind().unwrap();
    let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        fixture
            .source
            .entries()
            .local_inheritance_edges
            .records()
            .iter(),
        fixture
            .source
            .entries()
            .source_roots
            .values()
            .iter()
            .copied(),
        &bound,
    )
    .unwrap();
    let mut identities = identity_closure(&output);
    let decoded: hir::DecodedCanonicalExactTypeFactsV1 =
        decode_canonical(&encode(production.section().exact_facts()).unwrap()).unwrap();
    let facts = decoded.resolve(&mut identities).unwrap();
    facts.validate_semantics(&bound).unwrap();
    for record in production.section().inheritance().records() {
        graph
            .validate_nominal_domains(record.owner(), record.domains())
            .unwrap();
    }
    let object = fixture
        .source
        .entries()
        .representations
        .records()
        .iter()
        .find(|record| {
            matches!(
                record.shape(),
                hir::NominalRepresentationShapeV1::Object { .. }
            )
        })
        .unwrap();
    let exact =
        PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(object.owner()))
            .unwrap();
    assert!(graph.object_backing_relation(exact).is_some());

    let mut corrupt = facts.records().to_vec();
    let empty = corrupt
        .iter_mut()
        .find(|record| {
            matches!(
                record.kind(),
                hir::ExactTypeKindV1::Value {
                    zst: hir::ZstStatus::ZeroSized
                }
            )
        })
        .unwrap();
    *empty = hir::ExactTypeFactsV1::try_new(
        empty.exact(),
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::NonZero,
        },
        empty.gc(),
    )
    .unwrap();
    assert!(
        hir::CanonicalExactTypeFactsV1::try_new(corrupt)
            .unwrap()
            .validate_semantics(&bound)
            .is_err()
    );
}
