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
fn only_trusted_core_nominal_leaves_are_classified() {
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
        CoreClosedExactLeafClassifierV1 {
            leaves: vec![(unit, exact)],
        },
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
    )
    .unwrap()
}

fn foreign_type() -> scoop_identity::PersistentTypeId {
    let artifact = ConeCoordinate::new("tests", "foreign", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    CborIdentityRecord::<scoop_identity::PersistentTypeId, _>::from_key(
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
            0,
        ),
    )
    .unwrap()
    .id()
}
