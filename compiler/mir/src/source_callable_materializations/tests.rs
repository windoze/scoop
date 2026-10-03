use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableMaterializationContext,
    CallableTemplateOwner, CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactTypeKey, PackagePath, PersistentCallableApplicationId,
    PersistentExactTypeId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    SpecializationKey,
};

use super::*;

fn materialization(name: &str) -> CallableMaterialization {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    CallableMaterialization::new(
        CallableTemplateOwner::Function(
            PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
        ),
        CallableMaterializationContext::NoSubstitution,
    )
}

fn exact_unit() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}

fn signature() -> ExactCallableSignature {
    ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact_unit())
}

fn direct(
    function: FunctionId,
    materialization: CallableMaterialization,
) -> SourceCallableMaterialization {
    SourceCallableMaterialization::new(function, materialization, signature(), None).unwrap()
}

fn applied(
    source: CallableMaterialization,
) -> (
    CallableMaterialization,
    CborIdentityRecord<OdrGroupId, SpecializationKey>,
) {
    let CallableTemplateOwner::Function(owner) = source.template() else {
        unreachable!()
    };
    let application_key = CallableApplicationKey::for_function(
        owner,
        CallableInstantiationOwner::ExactNominalOwner(exact_unit()),
    );
    let application = PersistentCallableApplicationId::from_key(&application_key).unwrap();
    (
        CallableMaterialization::new(
            source.template(),
            CallableMaterializationContext::Application(application),
        ),
        CborIdentityRecord::from_key(SpecializationKey::Callable {
            application: application_key,
        })
        .unwrap(),
    )
}

#[test]
fn relation_sorts_and_queries_typed_function_locations() {
    let first = FunctionId::from_raw(1_u32.into());
    let second = FunctionId::from_raw(4_u32.into());
    let first_materialization = materialization("first");
    let relation = SourceCallableMaterializations::checked(vec![
        direct(second, materialization("second")),
        direct(first, first_materialization),
    ])
    .unwrap();

    assert_eq!(relation.len(), 2);
    assert_eq!(relation.iter().next().unwrap().function(), first);
    assert_eq!(
        relation.get(first).unwrap().materialization(),
        first_materialization
    );
    assert_eq!(
        relation.get(first).unwrap().signature_record().signature(),
        &signature()
    );
    assert!(matches!(
        relation.get(first).unwrap().signature_record().subject(),
        CallableSignatureSubject::Strong(CallableOwner::Function(_))
    ));
}

#[test]
fn substituted_callable_uses_its_callable_body_member_as_signature_subject() {
    let (materialization, group) = applied(materialization("applied"));
    let CallableMaterializationContext::Application(application) = materialization.context() else {
        unreachable!()
    };
    let entry = SourceCallableMaterialization::new(
        FunctionId::from_raw(2_u32.into()),
        materialization,
        signature(),
        Some(group.id()),
    )
    .unwrap();

    let member = entry.odr_member_record().unwrap();
    assert_eq!(member.key().group(), group.id());
    assert!(matches!(
        member.key().discriminator(),
        OdrMemberDiscriminator::CallableApplication(found) if *found == application
    ));
    assert_eq!(
        entry.signature_record().subject(),
        CallableSignatureSubject::odr(CallableOdrMemberId::from_key(member.key()).unwrap())
    );
}

#[test]
fn materialization_context_requires_exactly_its_odr_group_shape() {
    let source = materialization("context");
    let (application, group) = applied(source);
    assert_eq!(
        SourceCallableMaterialization::new(
            FunctionId::from_raw(0_u32.into()),
            source,
            signature(),
            Some(group.id()),
        )
        .unwrap_err(),
        SourceCallableMaterializationError::UnexpectedOdrGroup
    );
    assert_eq!(
        SourceCallableMaterialization::new(
            FunctionId::from_raw(0_u32.into()),
            application,
            signature(),
            None,
        )
        .unwrap_err(),
        SourceCallableMaterializationError::MissingOdrGroup
    );
}

#[test]
fn relation_rejects_duplicate_functions_and_materializations() {
    let first = FunctionId::from_raw(1_u32.into());
    let second = FunctionId::from_raw(4_u32.into());
    let shared = materialization("shared");
    assert_eq!(
        SourceCallableMaterializations::checked(vec![
            direct(first, materialization("first")),
            direct(first, materialization("second")),
        ])
        .unwrap_err(),
        SourceCallableMaterializationRelationError::DuplicateFunction { first: 0, index: 1 }
    );
    assert_eq!(
        SourceCallableMaterializations::checked(vec![
            direct(first, shared),
            direct(second, shared),
        ])
        .unwrap_err(),
        SourceCallableMaterializationRelationError::DuplicateMaterialization { first: 0, index: 1 }
    );

    let (first_application, group) = applied(materialization("application"));
    let application = first_application.context();
    let second_application =
        CallableMaterialization::new(materialization("other").template(), application);
    assert_eq!(
        SourceCallableMaterializations::checked(vec![
            SourceCallableMaterialization::new(
                first,
                first_application,
                signature(),
                Some(group.id()),
            )
            .unwrap(),
            SourceCallableMaterialization::new(
                second,
                second_application,
                signature(),
                Some(group.id()),
            )
            .unwrap(),
        ])
        .unwrap_err(),
        SourceCallableMaterializationRelationError::DuplicateSignatureSubject {
            first: 0,
            index: 1,
        }
    );
}
