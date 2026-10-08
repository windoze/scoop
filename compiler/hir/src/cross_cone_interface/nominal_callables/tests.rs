use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, Effect, ExactTypeKey, GcEffect,
    PackagePath, PersistentFunctionId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

use super::NominalExactLeafClassifierV1;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalSourceParameterShapesV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
};

mod signatures;

#[test]
fn only_known_nominal_leaves_are_classified() {
    let (classifier, unit, exact) = classifier();

    assert_eq!(
        classifier
            .classify(&SignatureTypeKey::Nominal(unit))
            .unwrap(),
        Some(exact)
    );
    assert_eq!(
        classifier
            .classify(&SignatureTypeKey::RawPointer(Box::new(
                SignatureTypeKey::Nominal(unit)
            )))
            .unwrap(),
        Some(
            scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::RawPointer(exact))
                .unwrap()
        )
    );
    assert_eq!(
        classifier
            .classify(&SignatureTypeKey::Nominal(foreign_type()))
            .unwrap(),
        None
    );
}

#[test]
fn source_any_requires_its_nominal_declaration() {
    let (classifier, _, unit) = classifier();
    let any = CoreBuiltinNominal::Any.identity_record().id();
    let exact =
        scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(any)).unwrap();
    assert_ne!(exact, unit);
    let signature = SignatureTypeKey::Nominal(any);
    let empty = NominalExactLeafClassifierV1::try_from_nominal_interfaces(&[]).unwrap();
    assert_eq!(empty.classify(&signature).unwrap(), None);
    assert_eq!(classifier.classify(&signature).unwrap(), Some(exact));
    let function = callable(
        signature,
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        GcEffect::Managed,
    );
    let classified = classifier.classify_callable(&function).unwrap().unwrap();
    assert_eq!(classified.signature().result(), exact);
    assert_eq!(classified.gc_effect(), GcEffect::Managed);
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
        NominalExactLeafClassifierV1::try_from_nominal_interfaces(nominals.records()).unwrap();
    let signature = SignatureTypeKey::Nominal(concrete);
    let expected =
        scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(concrete)).unwrap();
    assert_eq!(classifier.classify(&signature).unwrap(), Some(expected));
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
    let application = SignatureTypeKey::NominalApplication {
        origin: generic,
        arguments: scoop_identity::NonEmptyVec::from_first(SignatureTypeKey::Nominal(concrete), []),
    };
    let exact_application =
        scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::NominalApplication {
            origin: generic,
            arguments: scoop_identity::NonEmptyVec::from_first(expected, []),
        })
        .unwrap();
    assert_eq!(
        classifier.classify(&application).unwrap(),
        Some(exact_application)
    );
    let callable = callable(
        application.clone(),
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        GcEffect::Managed,
    );
    assert_eq!(
        classifier
            .classify_callable(&callable)
            .unwrap()
            .unwrap()
            .signature()
            .result(),
        exact_application
    );
    assert_eq!(
        NominalExactLeafClassifierV1::try_from_nominal_interfaces(&[])
            .unwrap()
            .classify(&application)
            .unwrap(),
        None
    );
    assert_eq!(
        classifier
            .classify(&SignatureTypeKey::NominalApplication {
                origin: generic,
                arguments: scoop_identity::NonEmptyVec::from_first(
                    SignatureTypeKey::Binder { depth: 0, index: 0 },
                    []
                ),
            })
            .unwrap(),
        None
    );
}

fn nominal(declaration: crate::SourceNominalId) -> crate::NominalInterfaceRecordV1 {
    if declaration
        == crate::SourceNominalId::Concrete(CoreBuiltinNominal::Any.identity_record().id())
    {
        return crate::nominal_interface_fixture::public_record(
            declaration,
            crate::PublicNominalKindV1::Class,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            crate::CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
            crate::CanonicalPersistentIdsV1::empty(),
            crate::CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            crate::CanonicalPersistentIdsV1::empty(),
            crate::NominalSourceShapeV1::Intrinsic(crate::NominalIntrinsicRepresentationV1::new(
                crate::IntrinsicTypeKind::Any,
            )),
        )
        .unwrap();
    }
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
                false,
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
    assert_eq!(classified.implementation(), classified.implementation());
    assert_eq!(classified.signature().effect(), Effect::Ordinary);
    assert_eq!(classified.signature().receiver().into_option(), None);
    assert!(classified.signature().parameters().is_empty());
    assert_eq!(classified.signature().result(), exact);
    assert_eq!(classified.gc_effect(), GcEffect::NoGc);
}

#[test]
fn suspend_callables_retain_the_source_effect_and_result() {
    let (classifier, unit, exact) = classifier();
    let callable = callable(
        SignatureTypeKey::Nominal(unit),
        Effect::Suspend,
        CallableImplementationV1::Scoop,
        GcEffect::Managed,
    );
    let classified = classifier.classify_callable(&callable).unwrap().unwrap();
    assert_eq!(classified.signature().effect(), Effect::Suspend);
    assert_eq!(classified.signature().result(), exact);
    assert_eq!(classified.direct_declaration(), None);
}

#[test]
fn param_free_source_extern_uses_its_provider_callable_entry() {
    let (classifier, unit, exact) = classifier();
    let callable = callable(
        SignatureTypeKey::Nominal(unit),
        Effect::Ordinary,
        CallableImplementationV1::SourceExternScoop,
        GcEffect::Managed,
    );
    let entry = classifier.classify_callable(&callable).unwrap().unwrap();
    assert_eq!(entry.signature().result(), exact);
    assert_eq!(entry.gc_effect(), GcEffect::Managed);
    assert!(entry.direct_declaration().is_some());
}

#[test]
fn native_leaf_contracts_keep_managed_provider_entries() {
    let (classifier, unit, _) = classifier();
    for implementation in [
        CallableImplementationV1::SourceExternC(
            scoop_identity::CAbiCallMode::NativeSafe,
            scoop_identity::CResultAdaptation::Direct,
        ),
        CallableImplementationV1::SourceExternScoop,
    ] {
        let callable = callable(
            SignatureTypeKey::Nominal(unit),
            Effect::Ordinary,
            implementation,
            GcEffect::NoGc,
        );
        let entry = classifier.classify_callable(&callable).unwrap().unwrap();
        assert_eq!(callable.effects().gc_effect(), GcEffect::NoGc);
        assert_eq!(entry.gc_effect(), GcEffect::Managed);
    }
}

#[test]
fn unresolved_signatures_remain_semantic_only() {
    let (classifier, _, _) = classifier();
    let callable = callable(
        SignatureTypeKey::Nominal(foreign_type()),
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        GcEffect::Managed,
    );
    assert_eq!(classifier.classify_callable(&callable).unwrap(), None);
}

fn classifier() -> (
    NominalExactLeafClassifierV1,
    scoop_identity::PersistentTypeId,
    scoop_identity::PersistentExactTypeId,
) {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let exact = scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(unit))
        .expect("core Unit exact identity is valid");
    (
        NominalExactLeafClassifierV1::try_from_nominal_interfaces(&[nominal(
            crate::SourceNominalId::Concrete(CoreBuiltinNominal::Any.identity_record().id()),
        )])
        .unwrap(),
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
            if implementation.is_c_extern() {
                CallableSafetyV1::Unsafe
            } else {
                CallableSafetyV1::Safe
            },
            gc_effect,
            implementation,
            CallableOperatorRoleV1::None,
            CallableInfixV1::Ordinary,
        )
        .unwrap(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        crate::CanonicalPersistentIdsV1::empty(),
        Vec::new(),
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
