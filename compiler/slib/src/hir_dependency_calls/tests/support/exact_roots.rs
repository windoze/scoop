use super::*;
use crate::hir_dependency_calls::applications::{signatures, validate_root};
use scoop_identity::{
    CallableOdrMemberId, CborIdentityRecord, GeneratedCallableKey, NonEmptyVec, OdrGroupId,
    OdrMemberDiscriminator, OdrMemberId, OdrMemberKey, OdrMemberRole, PendingIdentityValidation,
    PersistentGeneratedCallableId, PersistentTypeId, SourceNominalKind, SpecializationKey,
};
use scoop_mir::{CallableSignatureRecord, CallableSignatureSubject};

#[test]
fn exact_equality_root_uses_its_actual_odr_definition_without_lexical_substitution() {
    let fixture = Fixture::new();
    let exact = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(ExactTypeKey::Tuple(
        NonEmptyVec::from_first(fixture.unit, [fixture.unit]),
    ))
    .unwrap();
    let generated = CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(
        GeneratedCallableKey::DerivedEquality {
            exact_owner: exact.id(),
        },
    )
    .unwrap();
    let group = CborIdentityRecord::<OdrGroupId, _>::from_key(SpecializationKey::StructuralType {
        exact_type: exact.id(),
    })
    .unwrap();
    let member = CborIdentityRecord::<OdrMemberId, _>::from_key(
        OdrMemberKey::new(
            group.id(),
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::GeneratedCallable(generated.id()),
        )
        .unwrap(),
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_canonical_authority(exact.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(generated.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(group)
        .unwrap();
    pending
        .register_external_canonical_authority(member.clone())
        .unwrap();
    let identities = pending.finish().unwrap();
    let position = scoop_hir::concrete::ExecutableExpressionPosition {
        root: CallableMaterialization::new(
            CallableTemplateOwner::Generated(generated.id()),
            CallableMaterializationContext::NoSubstitution,
        ),
        expression_index: 2,
    };
    let mut foundation = CanonicalMirFoundation::empty();
    let boolean = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Boolean").unwrap(),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap();
    let boolean = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(boolean)).unwrap();
    let absent = signatures(&foundation, &identities, None).unwrap();
    assert!(matches!(
        validate_root(position, &fixture.strong, &absent, &identities),
        Err(CrossConeMirClosureRelationError::CallRoot { .. })
    ));
    foundation
        .set_callable_signatures(vec![CallableSignatureRecord::new(
            CallableSignatureSubject::odr(CallableOdrMemberId::from_key(member.key()).unwrap()),
            ExactCallableSignature::new(
                Effect::Ordinary,
                Some(exact.id()),
                vec![exact.id()],
                boolean,
            ),
        )])
        .unwrap();
    let present = signatures(&foundation, &identities, None).unwrap();
    validate_root(position, &fixture.strong, &present, &identities).unwrap();
}
