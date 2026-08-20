use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, DispatchSlotKey, ExactTypeKey, GeneratedCallableKey, LinkageClass,
    ObjectDefinitionPlanKey, OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole, PackagePath,
    PersistentCallableBodyId, PersistentDispatchSlotId, PersistentExactTypeId,
    PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, RuntimeIdentityRecord, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, SpecializationKey, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::encode;

use super::{ConeLirFoundation, ConeLirFoundationError};
use crate::CanonicalLirFoundation;

#[test]
fn accepts_and_preserves_strong_bodies_and_symbols() {
    let body = strong_body("entry");
    let request = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body.id()),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_callable_bodies(vec![body]).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());
    let expected = encode(&canonical).unwrap();

    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();

    assert_eq!(encode(&foundation).unwrap(), expected);
    assert_eq!(foundation.producer(), ConeIdentity::CORE);
    assert_eq!(foundation.as_canonical().counts().odr_members, 0);
}

#[test]
fn strong_profile_rejects_odr_group_and_member_tables_independently() {
    let (group, member) = odr_records();
    let expected_group = group.id();
    let mut with_group = CanonicalLirFoundation::empty();
    with_group.set_odr_groups(vec![group]).unwrap();
    assert_eq!(
        ConeLirFoundation::try_new(ConeIdentity::CORE, with_group)
            .unwrap()
            .require_strong(),
        Err(ConeLirFoundationError::OdrGroup(expected_group))
    );

    let expected_member = member.id();
    let mut with_member = CanonicalLirFoundation::empty();
    with_member.set_odr_members(vec![member]).unwrap();
    assert_eq!(
        ConeLirFoundation::try_new(ConeIdentity::CORE, with_member)
            .unwrap()
            .require_strong(),
        Err(ConeLirFoundationError::OdrMember(expected_member))
    );
}

#[test]
fn strong_profile_rejects_odr_callable_body_without_relying_on_odr_tables() {
    let member = callable_odr_member();
    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::odr(member)).unwrap();
    let expected = body.id();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_callable_bodies(vec![body]).unwrap();

    assert_eq!(
        ConeLirFoundation::try_new(ConeIdentity::CORE, canonical)
            .unwrap()
            .require_strong(),
        Err(ConeLirFoundationError::OdrCallableBody(expected))
    );
}

#[test]
fn strong_profile_rejects_odr_weak_symbol_without_relying_on_body_or_odr_tables() {
    let body = strong_body("weakBody");
    let key = PersistentSymbolKey::CallableBody(body.id());
    let request = PersistentSymbolRequest::new(key, LinkageClass::OdrWeak).unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());

    assert_eq!(
        ConeLirFoundation::try_new(ConeIdentity::CORE, canonical)
            .unwrap()
            .require_strong(),
        Err(ConeLirFoundationError::NonStrongSymbolRequest {
            key,
            linkage: LinkageClass::OdrWeak,
        })
    );
}

#[test]
fn strong_profile_rejects_explicit_odr_member_symbol_even_with_its_required_linkage() {
    let member = callable_odr_member().member();
    let key = PersistentSymbolKey::OdrMember(member);
    let request = PersistentSymbolRequest::new(key, LinkageClass::OdrWeak).unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());

    assert_eq!(
        ConeLirFoundation::try_new(ConeIdentity::CORE, canonical)
            .unwrap()
            .require_strong(),
        Err(ConeLirFoundationError::NonStrongSymbolRequest {
            key,
            linkage: LinkageClass::OdrWeak,
        })
    );
}

#[test]
fn strong_profile_rejects_template_support_hidden_symbols() {
    let slot = PersistentDispatchSlotId::from_key(&DispatchSlotKey::virtual_method(
        source_function("hiddenSlot"),
    ))
    .unwrap();
    let key = PersistentSymbolKey::DispatchSlot(slot);
    let request = PersistentSymbolRequest::new(key, LinkageClass::TemplateSupportHidden).unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());

    assert_eq!(
        ConeLirFoundation::try_new(ConeIdentity::CORE, canonical)
            .unwrap()
            .require_strong(),
        Err(ConeLirFoundationError::NonStrongSymbolRequest {
            key,
            linkage: LinkageClass::TemplateSupportHidden,
        })
    );
}

#[test]
fn strong_profile_rejects_odr_and_foreign_strong_definition_plans() {
    let odr_plan =
        CborIdentityRecord::from_key(ObjectDefinitionPlanKey::odr(callable_odr_member().member()))
            .unwrap();
    let expected_odr = odr_plan.id();
    let mut with_odr = CanonicalLirFoundation::empty();
    with_odr.set_definition_plans(vec![odr_plan]).unwrap();
    assert_eq!(
        ConeLirFoundation::try_new(ConeIdentity::CORE, with_odr)
            .unwrap()
            .require_strong(),
        Err(ConeLirFoundationError::OdrDefinitionPlan(expected_odr))
    );

    let body = strong_body("foreign");
    let foreign_plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let expected_plan = foreign_plan.id();
    let mut with_foreign = CanonicalLirFoundation::empty();
    with_foreign.set_callable_bodies(vec![body]).unwrap();
    with_foreign
        .set_definition_plans(vec![foreign_plan])
        .unwrap();
    assert_eq!(
        ConeLirFoundation::try_new(ConeIdentity::CORE, with_foreign),
        Err(ConeLirFoundationError::ForeignStrongDefinitionPlan {
            plan: expected_plan,
            expected: ConeIdentity::CORE,
            actual: ConeIdentity::SINGLE_FILE,
        })
    );
}

fn odr_records() -> (
    CborIdentityRecord<scoop_identity::OdrGroupId, SpecializationKey>,
    CborIdentityRecord<scoop_identity::OdrMemberId, OdrMemberKey>,
) {
    let exact = nominal_exact("Shape");
    let group =
        CborIdentityRecord::from_key(SpecializationKey::StructuralType { exact_type: exact })
            .unwrap();
    let member = CborIdentityRecord::from_key(
        OdrMemberKey::new(
            group.id(),
            OdrMemberRole::TypeDescriptor,
            OdrMemberDiscriminator::ExactType(exact),
        )
        .unwrap(),
    )
    .unwrap();
    (group, member)
}

fn callable_odr_member() -> scoop_identity::CallableOdrMemberId {
    let exact = nominal_exact("CallableShape");
    let generated =
        CborIdentityRecord::from_key(GeneratedCallableKey::CoroutineStart { result: exact })
            .unwrap();
    let group = scoop_identity::OdrGroupId::from_key(&SpecializationKey::StructuralType {
        exact_type: exact,
    })
    .unwrap();
    let member = OdrMemberKey::new(
        group,
        OdrMemberRole::CallableBody,
        OdrMemberDiscriminator::GeneratedCallable(generated.id()),
    )
    .unwrap();
    scoop_identity::CallableOdrMemberId::from_key(&member).unwrap()
}

fn strong_body(name: &str) -> RuntimeIdentityRecord<PersistentCallableBodyId> {
    let function = source_function(name);
    RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(function),
    ))
    .unwrap()
}

fn nominal_exact(name: &str) -> PersistentExactTypeId {
    let declaration = SourceDeclarationKey::nominal(
        declaration_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

fn source_function(name: &str) -> PersistentFunctionId {
    let declaration = SourceDeclarationKey::function(
        declaration_site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    PersistentFunctionId::from_source_declaration(&declaration).unwrap()
}

fn declaration_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
