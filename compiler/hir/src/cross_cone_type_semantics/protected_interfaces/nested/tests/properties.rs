use super::*;
use scoop_identity::{AccessorRole, PropertyOwner};

#[test]
fn nested_container_closes_runtime_property_and_private_setter_signatures() {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let owner = nested_class(&mut fixture, outer, "Nested");
    let ty = SignatureTypeKey::Nominal(nominal(fixture.unit));
    let get = fixture.accessor(owner, AccessorRole::Getter, ty.clone());
    let set = fixture.accessor(owner, AccessorRole::Setter, ty.clone());
    let (CallableTemplateOrigin::Accessor(get_id), CallableTemplateOrigin::Accessor(set_id)) =
        (get, set)
    else {
        unreachable!()
    };
    let PropertyOwner::Property(property) = fixture.accessors[&get_id].owner() else {
        unreachable!()
    };
    fixture.property_shapes.insert(
        property,
        ProtectedPropertySourceShapeV1 {
            getter: get_id,
            setter: Some((set_id, DeclaredVisibilityV1::Private)),
            representation: PropertyRepresentationV1::RuntimeAccessor,
        },
    );
    let getter = NominalSupportCallableInterfaceV1::try_new(
        get,
        fixture.access(owner, DeclaredVisibilityV1::Public),
        fixture
            .payload(owner, get, vec![], ty.clone())
            .source_signature,
    )
    .unwrap();
    let setter = NominalSupportCallableInterfaceV1::try_new(
        set,
        fixture.access(owner, DeclaredVisibilityV1::Private),
        fixture
            .payload(owner, set, vec![ty.clone()], ty.clone())
            .source_signature,
    )
    .unwrap();
    let property_record = NominalSupportPropertyInterfaceV1::try_new(
        property,
        fixture.access(owner, DeclaredVisibilityV1::Public),
        NominalSupportPropertyPayloadV1::Runtime {
            interface: NominalSourcePropertyPayloadV1::try_new(
                owner.source,
                ty,
                get_id,
                ProtectedPropertyMutabilityV1::ReadWrite {
                    setter: set_id,
                    setter_access: fixture.access(owner, DeclaredVisibilityV1::Private),
                },
                PropertyRepresentationV1::RuntimeAccessor,
                CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
            )
            .unwrap(),
        },
    )
    .unwrap();
    let interface = source(
        NominalInheritanceModalityV1::Open,
        vec![],
        vec![NestedSourceMemberRefV1::Property(property)],
        vec![],
        vec![
            NestedSourceSupportV1::Property(Box::new(property_record)),
            NestedSourceSupportV1::Callable(Box::new(getter)),
            NestedSourceSupportV1::Callable(Box::new(setter)),
        ],
    );
    let payload = payload(&mut fixture, owner, interface);
    let record = ProtectedNestedNominalInterfaceV1::try_new(
        owner.source,
        fixture.graph.access[&owner.source].clone(),
        payload,
    )
    .unwrap();
    let bytes = encode(&record).unwrap();
    let decoded: DecodedProtectedNestedNominalInterfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), record);
    let graph_source = fixture.graph.clone();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(graph_source.records.values(), &graph_source)
            .unwrap();
    let table = representations(&fixture);
    record
        .validate_source(&graph, &table, &mut fixture)
        .unwrap();
    fixture.property_shapes.get_mut(&property).unwrap().setter =
        Some((set_id, DeclaredVisibilityV1::Internal));
    assert!(matches!(
        record.validate_source(&graph, &table, &mut fixture),
        Err(NestedSourceSemanticError::Property(
            NominalSupportPropertySemanticError::Runtime(
                ProtectedPropertySemanticError::SourceShape
            )
        ))
    ));
}
