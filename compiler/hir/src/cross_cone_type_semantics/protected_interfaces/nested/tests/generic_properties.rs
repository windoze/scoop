use super::*;
use scoop_identity::{
    AccessorRole, CanonicalIdentifier, PersistentPropertyAccessorId, PersistentPropertyId,
    PropertyAccessorKey, PropertyOwner, SourceDeclarationKey,
};

#[test]
fn generic_property_keeps_source_access_without_fabricating_a_concrete_domain() {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let key = SourceDeclarationKey::nominal(
        site(&[outer.source]),
        CanonicalIdentifier::new("Generic").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let owner = SourceNominalId::from_source_declaration(&key).unwrap();
    let access = fixture.access(outer, DeclaredVisibilityV1::Protected);
    fixture.graph.keys.insert(owner, key);
    fixture
        .graph
        .origins
        .insert(owner, access.definition_origin().clone());
    fixture.graph.access.insert(owner, access.clone());
    let property_key = SourceDeclarationKey::property(
        site(&[outer.source, owner]),
        CanonicalIdentifier::new("value").unwrap(),
    );
    let property = PersistentPropertyId::from_source_declaration(&property_key).unwrap();
    let getter_key =
        PropertyAccessorKey::new(PropertyOwner::Property(property), AccessorRole::Getter);
    let setter_key =
        PropertyAccessorKey::new(PropertyOwner::Property(property), AccessorRole::Setter);
    let getter = PersistentPropertyAccessorId::from_key(&getter_key).unwrap();
    let setter = PersistentPropertyAccessorId::from_key(&setter_key).unwrap();
    fixture.accessors.insert(getter, getter_key);
    fixture.accessors.insert(setter, setter_key);
    fixture.declarations.insert(
        CallableTemplateOrigin::Accessor(getter),
        property_key.clone(),
    );
    fixture
        .declarations
        .insert(CallableTemplateOrigin::Accessor(setter), property_key);
    let ty = SignatureTypeKey::Binder { depth: 0, index: 0 };
    fixture.property_types.insert(property, ty.clone());
    fixture.property_shapes.insert(
        property,
        ProtectedPropertySourceShapeV1 {
            getter,
            setter: Some((setter, DeclaredVisibilityV1::Private)),
            representation: PropertyRepresentationV1::RuntimeAccessor,
        },
    );
    let member_access = |visibility| {
        DeclarationAccessSourceV1::try_new(
            visibility,
            vec![outer.source, owner],
            access.definition_origin().clone(),
        )
        .unwrap()
    };
    let property_payload = NominalSourcePropertyPayloadV1::try_new(
        owner,
        ty.clone(),
        getter,
        ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access: member_access(DeclaredVisibilityV1::Private),
        },
        PropertyRepresentationV1::RuntimeAccessor,
        CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
    )
    .unwrap();
    let property_record = NominalSupportPropertyInterfaceV1::try_new(
        property,
        member_access(DeclaredVisibilityV1::Protected),
        NominalSupportPropertyPayloadV1::Runtime {
            interface: property_payload,
        },
    )
    .unwrap();
    let effects = CallableSourceEffectsV1::try_new(
        scoop_identity::Effect::Ordinary,
        CallableSafetyV1::Safe,
        scoop_identity::GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap();
    let make_accessor = |id, parameters, result, visibility| {
        NominalSupportCallableInterfaceV1::try_new(
            CallableTemplateOrigin::Accessor(id),
            member_access(visibility),
            NominalSourceCallablePayloadV1::try_new(
                CallableTemplateOrigin::Accessor(id),
                owner,
                CanonicalBinderListV1::try_new(vec![]).unwrap(),
                CanonicalSourceParameterShapesV1::try_new(parameters).unwrap(),
                result,
                effects,
                CallableModalityV1::Final,
                CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let get = make_accessor(getter, vec![], ty.clone(), DeclaredVisibilityV1::Protected);
    let set = make_accessor(
        setter,
        vec![SourceParameterShapeV1::new(
            CanonicalIdentifier::new("value").unwrap(),
            ty,
        )],
        SignatureTypeKey::Nominal(nominal(fixture.unit)),
        DeclaredVisibilityV1::Private,
    );
    let mut interface = source(
        NominalInheritanceModalityV1::Open,
        vec![],
        vec![NestedSourceMemberRefV1::Property(property)],
        vec![],
        vec![
            NestedSourceSupportV1::Property(Box::new(property_record.clone())),
            NestedSourceSupportV1::Callable(Box::new(get)),
            NestedSourceSupportV1::Callable(Box::new(set)),
        ],
    );
    interface.type_parameters = CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        CanonicalIdentifier::new("T").unwrap(),
        TypeParameterBoundsV1::Unconstrained,
    )])
    .unwrap();
    fixture.nominal_sources.insert(owner, interface.clone());
    let payload = ProtectedNestedNominalPayloadV1::try_new(owner, interface).unwrap();
    let record = ProtectedNestedNominalInterfaceV1::try_new(owner, access, payload).unwrap();
    let decoded: DecodedProtectedNestedNominalInterfaceV1 =
        decode_canonical(&encode(&record).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), record);
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        graph_source.records.values(),
        graph_source.keys.keys().copied(),
        &graph_source,
    )
    .unwrap();
    record
        .validate_source(
            &graph,
            &CanonicalNominalRepresentationSupportV1::default(),
            &mut fixture,
        )
        .unwrap();
    let CheckedNominalSupportPropertySourceV1::Runtime(checked) = property_record
        .validate_source(&graph, &mut fixture)
        .unwrap()
    else {
        panic!("runtime property required")
    };
    assert_eq!(
        checked.access_proof(),
        NominalSupportPropertyAccessProofV1::GenericSourceMetadata
    );
    assert!(
        graph
            .replay_declaration_access(checked.declaration_access())
            .is_err()
    );
    fixture.property_shapes.get_mut(&property).unwrap().setter =
        Some((setter, DeclaredVisibilityV1::Internal));
    assert!(matches!(
        record.validate_source(
            &graph,
            &CanonicalNominalRepresentationSupportV1::default(),
            &mut fixture
        ),
        Err(NestedSourceSemanticError::Property(
            NominalSupportPropertySemanticError::Runtime(
                ProtectedPropertySemanticError::SourceShape
            )
        ))
    ));
}
