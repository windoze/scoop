use super::*;
use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableMaterialization,
    CallableMaterializationContext, CallableTemplateOwner, CanonicalIdentifier, CborIdentityRecord,
    ConeCoordinate, CoreBuiltinNominal, DeclarationScope, DefinitionOrigin,
    DefinitionOriginSubject, DefinitionOwnerChain, ExactTypeKey, LocalValueKey, LocalValueSelector,
    NonEmptyVec, NormalizedSourcePath, PackagePath, PersistentCallableApplicationId,
    PersistentExactTypeId, PersistentGenericFunctionId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceSpan,
};

#[test]
fn imported_generic_parameters_require_the_template_source_and_points() {
    let provider = ConeCoordinate::new("example", "provider", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let consumer = ConeCoordinate::new("example", "consumer", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let text = "fun <T> identity(value: T): T = value";
    let source = SourceIdentity::new(
        provider,
        NormalizedSourcePath::new("identity.scoop").unwrap(),
    )
    .unwrap();
    let span = SourceSpan::new(16, 21).unwrap();
    let origin = DefinitionOrigin::new(
        source.clone(),
        span,
        &SourceContextKey::File {
            source: source.clone(),
        },
    )
    .unwrap();
    let template = CborIdentityRecord::<PersistentGenericFunctionId, _>::from_key(
        SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                provider,
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
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    let application =
        PersistentCallableApplicationId::from_key(&CallableApplicationKey::for_generic_function(
            template.id(),
            CallableInstantiationOwner::NoOwner,
            NonEmptyVec::from_first(exact, []),
        ))
        .unwrap();
    let local = CborIdentityRecord::from_key(LocalValueKey::new(
        CallableMaterialization::new(
            CallableTemplateOwner::GenericFunction(template.id()),
            CallableMaterializationContext::Application(application),
        ),
        LocalValueSelector::Parameter {
            declaration_index: 0,
        },
    ))
    .unwrap();
    let subject = DefinitionOriginSubject::LocalValue(local.id());
    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_definition_origins(vec![DefinitionOriginRecord::new(
            DefinitionOriginSubject::GenericFunction(template.id()),
            origin.clone(),
        )])
        .unwrap();
    foundation.set_generic_functions(vec![template]).unwrap();
    let sources = [SourceRecord::from_utf8(source.clone(), text, [16, 21]).unwrap()];
    let validate = |origin: &DefinitionOrigin, dependencies: &[&CanonicalHirFoundation]| {
        let path = WirePath::root().field(29);
        let mut requirements = OriginRequirements::new(&[], &[], &[], 1, &path).unwrap();
        requirements.local_value(&local, &path).unwrap();
        validate_records(
            consumer,
            &sources,
            requirements,
            &[DefinitionOriginRecord::new(subject, origin.clone())],
            dependencies,
        )
    };
    validate(&origin, &[&foundation]).unwrap();
    assert!(matches!(
        validate(&origin, &[]),
        Err(HirFoundationValidationError::Origin(
            DefinitionOriginValidationError::MissingSourceAnchor { subject: actual }
        )) if actual == subject
    ));
    let wrong_source = SourceIdentity::new(
        consumer,
        NormalizedSourcePath::new("consumer.scoop").unwrap(),
    )
    .unwrap();
    let wrong = DefinitionOrigin::new(
        wrong_source.clone(),
        span,
        &SourceContextKey::File {
            source: wrong_source,
        },
    )
    .unwrap();
    assert!(matches!(
        validate(&wrong, &[&foundation]),
        Err(HirFoundationValidationError::Origin(
            DefinitionOriginValidationError::SourceMismatch { subject: actual, expected, .. }
        )) if actual == subject && *expected == source
    ));
    let wrong = DefinitionOrigin::new(
        source.clone(),
        SourceSpan::new(17, 21).unwrap(),
        &SourceContextKey::File { source },
    )
    .unwrap();
    assert!(matches!(
        validate(&wrong, &[&foundation]),
        Err(HirFoundationValidationError::Origin(
            DefinitionOriginValidationError::MissingPoint { subject: actual, byte_offset: 17, .. }
        )) if actual == subject
    ));
}
