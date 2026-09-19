use super::*;
use scoop_identity::{CanonicalIdentifier, EnumVariantIdentityKey, PersistentEnumVariantId};

#[test]
fn nested_enum_shape_closes_variant_constructor_through_typed_variant_protocol() {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let owner = fixture
        .graph
        .add("Choice", SourceNominalKind::Enum, &[outer]);
    fixture
        .graph
        .visibility(owner, DeclaredVisibilityV1::Protected);
    let key = EnumVariantIdentityKey::source(
        &fixture.graph.keys[&owner.source],
        CanonicalIdentifier::new("None").unwrap(),
    )
    .unwrap();
    let variant = PersistentEnumVariantId::from_key(&key).unwrap();
    let shape =
        EnumSourceVariantV1::try_new(variant, EnumSourceVariantStyleV1::Unit, vec![]).unwrap();
    fixture
        .variants
        .insert(variant, (key.clone(), shape.clone()));
    let variant_representation =
        EnumRepresentationVariantV1::try_new(&key, vec![], ExactTypeGcV1::GcFree).unwrap();
    fixture.graph.representations.insert(
        nominal(owner),
        NominalRepresentationSupportV1::try_new(
            &fixture.graph.keys[&owner.source],
            fixture.graph.access[&owner.source].clone(),
            NominalRepresentationShapeV1::Enum {
                variants: vec![variant_representation],
            },
        )
        .unwrap(),
    );
    let effects = CallableSourceEffectsV1::try_new(
        scoop_identity::Effect::Ordinary,
        CallableSafetyV1::Safe,
        scoop_identity::GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap();
    let callable_payload = NominalSourceCallablePayloadV1::try_new(
        CallableTemplateOrigin::VariantConstructor(variant),
        owner.source,
        CanonicalBinderListV1::try_new(vec![]).unwrap(),
        CanonicalSourceParameterShapesV1::try_new(vec![]).unwrap(),
        SignatureTypeKey::Nominal(nominal(owner)),
        effects,
        CallableModalityV1::Final,
        CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
    )
    .unwrap();
    let variant_record = NominalSupportCallableInterfaceV1::try_new(
        CallableTemplateOrigin::VariantConstructor(variant),
        fixture.access(owner, DeclaredVisibilityV1::Public),
        callable_payload,
    )
    .unwrap();
    let mut interface = source(
        NominalInheritanceModalityV1::Final,
        vec![],
        vec![],
        vec![],
        vec![NestedSourceSupportV1::Callable(Box::new(variant_record))],
    );
    interface.kind = PublicNominalKindV1::Enum;
    interface.source_shape =
        NominalSourceShapeV1::Enum(EnumSourceShapeV1::try_new(vec![shape]).unwrap());
    let payload = payload(&mut fixture, owner, interface);
    let record = ProtectedNestedNominalInterfaceV1::try_new(
        owner.source,
        fixture.graph.access[&owner.source].clone(),
        payload,
    )
    .unwrap();
    let decoded: DecodedProtectedNestedNominalInterfaceV1 =
        decode_canonical(&encode(&record).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), record);
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    record
        .validate_source(
            &graph,
            &representations(&fixture),
            &mut fixture,
            &mut meter(),
        )
        .unwrap();
    let mut missing = record.payload().source_interface().clone();
    missing.source_support = CanonicalNestedSourceSupportV1::default();
    assert!(matches!(
        ProtectedNestedNominalPayloadV1::try_new(owner.source, missing, record.payload().support()),
        Err(NestedSourceBuildError::ReferenceClosure)
    ));
}
