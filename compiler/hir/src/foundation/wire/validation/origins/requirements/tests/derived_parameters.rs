use scoop_identity::{
    CallableMaterializationContext, CoreBuiltinNominal, DefinitionOrigin, DefinitionOriginRecord,
    ExactTypeKey, LocalValueKey, NormalizedSourcePath, PersistentExactTypeId, SourceContextKey,
    SourceIdentity, SourceSpan,
};

use super::*;
use crate::foundation::wire::validation::origins::records::validate_records;

fn application() -> GeneratedCallableRecord {
    let exact = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    CborIdentityRecord::from_key(GeneratedCallableKey::DerivedEquality {
        exact_owner: exact.id(),
    })
    .unwrap()
}

fn local(owner: CallableTemplateOwner, selector: LocalValueSelector) -> LocalValueRecord {
    CborIdentityRecord::from_key(LocalValueKey::new(
        CallableMaterialization::new(owner, CallableMaterializationContext::NoSubstitution),
        selector,
    ))
    .unwrap()
}

#[test]
fn derived_parameters_reject_fabricated_source_origins() {
    let records = [application()];
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("equality.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    let origin = DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap();
    let path = WirePath::root().field(29);

    for selector in [
        LocalValueSelector::This,
        LocalValueSelector::Parameter {
            declaration_index: 0,
        },
    ] {
        let local = local(CallableTemplateOwner::Generated(records[0].id()), selector);
        let subject = DefinitionOriginSubject::LocalValue(local.id());
        for fabricated in [false, true] {
            let mut requirements = OriginRequirements::new(&records, 1, &path).unwrap();
            requirements.local_value(&local, &path).unwrap();
            let origins = if fabricated {
                vec![DefinitionOriginRecord::new(subject, origin.clone())]
            } else {
                Vec::new()
            };
            let result = validate_records(ConeIdentity::CORE, &[], requirements, &origins);
            if fabricated {
                assert!(matches!(result, Err(HirFoundationValidationError::Origin(
                    DefinitionOriginValidationError::UnexpectedSubject { subject: actual }
                )) if actual == subject));
            } else {
                result.unwrap();
            }
        }
    }
}

#[test]
fn only_the_derived_receiver_and_sole_parameter_are_synthetic() {
    let records = [application()];
    let local = local(
        CallableTemplateOwner::Generated(records[0].id()),
        LocalValueSelector::Parameter {
            declaration_index: 1,
        },
    );
    let path = WirePath::root().field(29);

    let mut requirements = OriginRequirements::new(&records, 1, &path).unwrap();
    assert!(matches!(
        requirements.local_value(&local,  &path),
        Err(HirFoundationValidationError::Origin(DefinitionOriginValidationError::MissingSourceAnchor { subject }))
            if subject == DefinitionOriginSubject::LocalValue(local.id())
    ));
}

#[test]
fn ordinary_receiver_still_requires_a_source_origin() {
    let key = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("ordinary_receiver").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let local = local(
        CallableTemplateOwner::Function(
            PersistentFunctionId::from_source_declaration(&key).unwrap(),
        ),
        LocalValueSelector::This,
    );
    let path = WirePath::root().field(29);

    let mut requirements = OriginRequirements::new(&[], 1, &path).unwrap();
    requirements.local_value(&local, &path).unwrap();
    assert!(matches!(
        validate_records(ConeIdentity::CORE, &[], requirements, &[]),
        Err(HirFoundationValidationError::Origin(DefinitionOriginValidationError::MissingSubject { subject }))
            if subject == DefinitionOriginSubject::LocalValue(local.id())
    ));
}
