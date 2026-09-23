use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, Effect, ExactTypeKey, GcEffect,
    PackagePath, PersistentFunctionId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

use super::CoreClosedExactLeafClassifierV1;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalSourceParameterShapesV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
};

#[test]
fn only_known_nominal_leaves_are_classified() {
    let (classifier, unit, exact) = classifier();

    assert_eq!(
        classifier.classify(&SignatureTypeKey::Nominal(unit)),
        Some(exact)
    );
    assert_eq!(
        classifier.classify(&SignatureTypeKey::RawPointer(Box::new(
            SignatureTypeKey::Nominal(unit)
        ))),
        None
    );
    assert_eq!(
        classifier.classify(&SignatureTypeKey::Nominal(foreign_type())),
        None
    );
}

#[test]
fn shared_nominal_surface_supplies_exact_leaves_without_a_core_sidecar() {
    use crate::{CanonicalNominalInterfacesV1, SourceNominalId};

    let concrete = foreign_type();
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let generic = CborIdentityRecord::<scoop_identity::PersistentGenericTypeId, _>::from_key(
        foreign_nominal_key(1),
    )
    .unwrap()
    .id();
    let nominals = CanonicalNominalInterfacesV1::try_new(vec![
        nominal(SourceNominalId::Concrete(concrete)),
        nominal(SourceNominalId::Concrete(unit)),
        nominal(SourceNominalId::GenericTemplate(generic)),
    ])
    .unwrap();
    let classifier =
        CoreClosedExactLeafClassifierV1::try_from_nominal_interfaces(nominals.records()).unwrap();
    let signature = SignatureTypeKey::Nominal(concrete);
    let expected =
        scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(concrete)).unwrap();
    assert_eq!(classifier.classify(&signature), Some(expected));
    let function = callable(
        signature,
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        GcEffect::Managed,
    );
    assert_eq!(
        classifier
            .classify_callable(&function)
            .unwrap()
            .unwrap()
            .signature()
            .result(),
        expected
    );
    assert_eq!(
        classifier.classify(&SignatureTypeKey::NominalApplication {
            origin: generic,
            arguments: scoop_identity::NonEmptyVec::new(vec![SignatureTypeKey::Nominal(concrete)])
                .unwrap(),
        }),
        None
    );
}

fn nominal(declaration: crate::SourceNominalId) -> crate::NominalInterfaceRecordV1 {
    let binders = match declaration {
        crate::SourceNominalId::Concrete(_) => Vec::new(),
        crate::SourceNominalId::GenericTemplate(_) => vec![crate::TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            crate::TypeParameterBoundsV1::Unconstrained,
        )],
    };
    crate::nominal_interface_fixture::public_record(
        declaration,
        crate::PublicNominalKindV1::Struct,
        CanonicalBinderListV1::try_new(binders).unwrap(),
        crate::CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        crate::CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        crate::CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        crate::CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        crate::NominalSourceShapeV1::Struct(
            crate::StructSourceShapeV1::try_new(
                Vec::new(),
                crate::NominalCLayoutPolicyV1::Ordinary,
            )
            .unwrap(),
        ),
    )
    .unwrap()
}

#[test]
fn ordinary_scoop_function_with_core_closed_signature_is_executable() {
    let (classifier, unit, exact) = classifier();
    let callable = callable(
        SignatureTypeKey::Nominal(unit),
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        GcEffect::NoGc,
    );

    let classified = classifier
        .classify_callable(&callable)
        .unwrap()
        .expect("ordinary core-closed callable is executable");
    assert_eq!(
        classified.declaration().implementation(),
        classified.implementation()
    );
    assert_eq!(classified.signature().effect(), Effect::Ordinary);
    assert_eq!(classified.signature().receiver().into_option(), None);
    assert!(classified.signature().parameters().is_empty());
    assert_eq!(classified.signature().result(), exact);
    assert_eq!(classified.gc_effect(), GcEffect::NoGc);
}

#[test]
fn unsupported_execution_and_signature_shapes_remain_semantic_only() {
    let (classifier, unit, _) = classifier();
    let cases = [
        callable(
            SignatureTypeKey::Nominal(unit),
            Effect::Suspend,
            CallableImplementationV1::Scoop,
            GcEffect::Managed,
        ),
        callable(
            SignatureTypeKey::Nominal(unit),
            Effect::Ordinary,
            CallableImplementationV1::SourceExternScoop,
            GcEffect::Managed,
        ),
        callable(
            SignatureTypeKey::Nominal(foreign_type()),
            Effect::Ordinary,
            CallableImplementationV1::Scoop,
            GcEffect::Managed,
        ),
    ];

    for callable in cases {
        assert_eq!(classifier.classify_callable(&callable).unwrap(), None);
    }
}

fn classifier() -> (
    CoreClosedExactLeafClassifierV1,
    scoop_identity::PersistentTypeId,
    scoop_identity::PersistentExactTypeId,
) {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let exact = scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(unit))
        .expect("core Unit exact identity is valid");
    (
        CoreClosedExactLeafClassifierV1::try_from_nominal_interfaces(&[]).unwrap(),
        unit,
        exact,
    )
}

fn callable(
    result: SignatureTypeKey,
    effect: Effect,
    implementation: CallableImplementationV1,
    gc_effect: GcEffect,
) -> CallableInterfaceRecordV1 {
    let artifact = ConeCoordinate::new("tests", "consumer", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let function =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                artifact,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("run").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Function(function.id()),
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        result,
        CallableSourceEffectsV1::try_new(
            effect,
            CallableSafetyV1::Safe,
            gc_effect,
            implementation,
            CallableOperatorRoleV1::None,
            CallableInfixV1::Ordinary,
        )
        .unwrap(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        crate::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap()
}

fn foreign_type() -> scoop_identity::PersistentTypeId {
    CborIdentityRecord::<scoop_identity::PersistentTypeId, _>::from_key(foreign_nominal_key(0))
        .unwrap()
        .id()
}

fn foreign_nominal_key(type_parameter_count: u32) -> SourceDeclarationKey {
    let artifact = ConeCoordinate::new("tests", "foreign", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            artifact,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Foreign").unwrap(),
        SourceNominalKind::Struct,
        type_parameter_count,
    )
}
