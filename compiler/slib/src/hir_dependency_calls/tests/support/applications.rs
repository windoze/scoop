use super::*;
use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableOdrMemberId, CborIdentityRecord,
    NonEmptyVec, OdrGroupId, OdrMemberDiscriminator, OdrMemberId, OdrMemberKey, OdrMemberRole,
    PendingIdentityValidation, PersistentCallableApplicationId, PersistentGenericFunctionId,
    SignatureTypeKey, SpecializationKey,
};
use scoop_mir::{CallableSignatureRecord, CallableSignatureSubject};

#[test]
fn application_calls_and_roots_use_the_local_odr_body_signature() {
    let fixture = Fixture::new();
    let template = CborIdentityRecord::<PersistentGenericFunctionId, _>::from_key(
        SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                fixture.provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("identity").unwrap(),
            1,
            None,
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
        ),
    )
    .unwrap();
    let application = CborIdentityRecord::<PersistentCallableApplicationId, _>::from_key(
        CallableApplicationKey::for_generic_function(
            template.id(),
            CallableInstantiationOwner::NoOwner,
            NonEmptyVec::from_first(fixture.unit, []),
        ),
    )
    .unwrap();
    let group = CborIdentityRecord::<OdrGroupId, _>::from_key(SpecializationKey::Callable {
        application: application.key().clone(),
    })
    .unwrap();
    let member = CborIdentityRecord::<OdrMemberId, _>::from_key(
        OdrMemberKey::new(
            group.id(),
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::CallableApplication(application.id()),
        )
        .unwrap(),
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    for provider in [ConeIdentity::CORE, fixture.provider, fixture.current] {
        pending.register_authority(provider).unwrap();
    }
    pending
        .register_external_canonical_authority(template.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(application.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(group)
        .unwrap();
    pending
        .register_external_canonical_authority(member.clone())
        .unwrap();
    let identities = pending.finish().unwrap();
    let mut foundation = CanonicalMirFoundation::empty();
    foundation
        .set_callable_signatures(vec![CallableSignatureRecord::new(
            CallableSignatureSubject::odr(CallableOdrMemberId::from_key(member.key()).unwrap()),
            ExactCallableSignature::new(Effect::Ordinary, None, vec![fixture.unit], fixture.unit),
        )])
        .unwrap();
    let bridge =
        CrossConeMirBridgeSectionV1::try_new(fixture.current, &foundation, vec![], vec![]).unwrap();
    let binding = PersistentExportBindingId::from_key(&ExportBindingKey::new(
        fixture.provider,
        PackagePath::root(),
        CanonicalIdentifier::new("identity").unwrap(),
        BindingTarget::function(template.key()).unwrap(),
    ))
    .unwrap();
    let direct = fixture.site(0, vec![fixture.unit]);
    let interface = |root, arguments: Vec<_>| {
        let site = HirDependencyCallSiteV1::try_new_with_instantiation(
            scoop_hir::concrete::ExecutableExpressionPosition {
                root,
                expression_index: 0,
            },
            direct.origin().clone(),
            arguments,
            fixture.unit,
            direct.reason().clone(),
            direct.receiver(),
            HirDependencyCallInstantiationV1::Application(application.id()),
        )
        .unwrap();
        let reference = ExternalHirReferenceV1::try_new(
            fixture.provider,
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::GenericFunction(template.id())),
            CanonicalExternalHirReferenceRolesV1::try_new(vec![
                ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            ])
            .unwrap(),
            CanonicalDependencyBindingWitnessesV1::try_new(vec![DependencyBindingWitnessV1::new(
                ReexportRouteV1::try_new(
                    fixture.provider,
                    vec![ReexportRouteHopV1::new(fixture.provider, binding)],
                )
                .unwrap(),
            )])
            .unwrap(),
            CanonicalHirDependencyCallSitesV1::try_new(vec![site]).unwrap(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(direct_callable(&reference).unwrap(), None);
        interface::empty_interface(vec![reference])
    };
    let validate = |interface: &_, foundation: &_| {
        validate_executable_hir_calls(
            interface,
            &fixture.strong,
            &bridge,
            None,
            foundation,
            &identities,
        )
    };
    let strong_root = direct.position().root;
    validate(&interface(strong_root, vec![fixture.unit]), &foundation).unwrap();
    assert!(matches!(
        validate(&interface(strong_root, Vec::new()), &foundation),
        Err(CrossConeMirClosureRelationError::CallSignature { .. })
    ));
    assert!(matches!(
        validate(&interface(strong_root, vec![fixture.unit]), &CanonicalMirFoundation::empty()),
        Err(CrossConeMirClosureRelationError::MissingMirApplication { application: actual, .. })
            if actual == application.id()
    ));
    let generic_root = CallableMaterialization::new(
        CallableTemplateOwner::GenericFunction(template.id()),
        CallableMaterializationContext::Application(application.id()),
    );
    validate(&interface(generic_root, vec![fixture.unit]), &foundation).unwrap();
    let wrong_root = CallableMaterialization::new(strong_root.template(), generic_root.context());
    assert!(matches!(
        validate(&interface(wrong_root, vec![fixture.unit]), &foundation),
        Err(CrossConeMirClosureRelationError::CallRoot { .. })
    ));
}
