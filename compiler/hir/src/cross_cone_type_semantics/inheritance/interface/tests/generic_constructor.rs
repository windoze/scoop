use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;
use scoop_identity::{
    CanonicalIdentifier, NonEmptyVec, PersistentConstructorId, PersistentGenericTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceNominalKind,
};

#[test]
fn generic_constructor_source_metadata_cannot_acquire_a_concrete_inheritance_entry() {
    let mut fixture = Fixture::default();
    let key = SourceDeclarationKey::nominal(
        site(&[]),
        CanonicalIdentifier::new("GenericOwner").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let generic = PersistentGenericTypeId::from_source_declaration(&key).unwrap();
    let owner = SourceNominalId::GenericTemplate(generic);
    fixture.graph.keys.insert(owner, key);
    let key = SourceDeclarationKey::constructor(site(&[owner]), vec![]);
    let constructor = PersistentConstructorId::from_source_declaration(&key).unwrap();
    fixture
        .declarations
        .insert(CallableTemplateOrigin::Constructor(constructor), key);
    let effects = CallableSourceEffectsV1::try_new(
        scoop_identity::Effect::Ordinary,
        CallableSafetyV1::Safe,
        scoop_identity::GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap();
    let payload = NominalSourceCallablePayloadV1::try_new(
        CallableTemplateOrigin::Constructor(constructor),
        owner,
        CanonicalBinderListV1::try_new(vec![]).unwrap(),
        CanonicalSourceParameterShapesV1::try_new(vec![]).unwrap(),
        SignatureTypeKey::NominalApplication {
            origin: generic,
            arguments: NonEmptyVec::from_first(SignatureTypeKey::Binder { depth: 0, index: 0 }, []),
        },
        effects,
        CallableModalityV1::Final,
        CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
    )
    .unwrap();
    let access = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Public,
        vec![owner],
        fixture.graph.origins[&fixture.unit.source].clone(),
    )
    .unwrap();
    let source =
        NominalSupportConstructorInterfaceV1::try_new(constructor, access, payload).unwrap();
    assert!(matches!(
        InheritanceConstructorInterfaceV1::try_new(source.clone()),
        Err(InheritanceInterfaceBuildError::ConstructorGeneric)
    ));
    let decoded: DecodedInheritanceConstructorInterfaceV1 =
        decode_canonical(&encode(&source).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(InheritanceInterfaceResolutionError::Build(
            InheritanceInterfaceBuildError::ConstructorGeneric
        ))
    ));
}
