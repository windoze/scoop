use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, LinkageClass,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentCallableBodyId, PersistentFunctionId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    RuntimeIdentityRecord, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::CanonicalLirFoundation;

#[test]
fn derives_primary_and_every_atom_boundary_from_typed_plans() {
    let fixture = fixture(true);
    let surface = ObjectSymbolSurfaceV1::from_foundation(&fixture.foundation).unwrap();
    assert_eq!(surface.plans().len(), 1);
    assert_eq!(surface.plan(fixture.plan.id()), Some(&surface.plans()[0]));

    let plan = &surface.plans()[0];
    assert_eq!(plan.definition_plan(), fixture.plan.id());
    assert_eq!(plan.primary_atom(), fixture.primary.id());
    assert_eq!(
        plan.primary_symbol().key(),
        PersistentSymbolKey::CallableBody(fixture.body.id())
    );
    assert_eq!(plan.primary_symbol().linkage(), LinkageClass::ConeStrong);
    assert_eq!(plan.atom_boundaries().len(), 2);

    for atom in [fixture.primary.id(), fixture.eh_frame.id()] {
        let boundary = plan
            .atom_boundaries()
            .iter()
            .find(|boundary| boundary.atom() == atom)
            .unwrap();
        assert_eq!(
            boundary.start().key(),
            PersistentSymbolKey::DefinitionBoundaryStart(atom)
        );
        assert_eq!(
            boundary.end().key(),
            PersistentSymbolKey::DefinitionBoundaryEnd(atom)
        );
        assert_eq!(boundary.start().linkage(), LinkageClass::ConeStrong);
        assert_eq!(boundary.end().linkage(), LinkageClass::ConeStrong);
    }
}

#[test]
fn wire_reader_only_returns_the_independently_rebuilt_surface() {
    let fixture = fixture(true);
    let surface = ObjectSymbolSurfaceV1::from_foundation(&fixture.foundation).unwrap();
    let encoded = encode(&surface).unwrap();
    let decoded: DecodedObjectSymbolSurfaceV1 = decode_canonical(&encoded).unwrap();

    assert_eq!(decoded.validate(&fixture.foundation), Ok(surface));
}

#[test]
fn wire_reader_rejects_non_closed_definition_and_boundary_records() {
    let fixture = fixture(true);
    let surface = ObjectSymbolSurfaceV1::from_foundation(&fixture.foundation).unwrap();
    let encoded = encode(&surface).unwrap();
    assert_eq!(encoded[0], 0x81);
    assert_eq!(encoded[1], 0xa6);

    let mut missing_definition_field = encoded.clone();
    missing_definition_field[1] = 0xa5;
    assert!(decode_canonical::<DecodedObjectSymbolSurfaceV1>(&missing_definition_field,).is_err());

    let boundary_map = encoded
        .windows(2)
        .position(|window| window == [0x06, 0x82])
        .map(|index| index + 2)
        .unwrap();
    assert_eq!(encoded[boundary_map], 0xa4);
    let mut extra_boundary_field = encoded;
    extra_boundary_field[boundary_map] = 0xa5;
    assert!(decode_canonical::<DecodedObjectSymbolSurfaceV1>(&extra_boundary_field,).is_err());
}

#[test]
fn refuses_a_definition_without_its_foundation_primary_symbol() {
    let fixture = fixture(false);
    assert_eq!(
        ObjectSymbolSurfaceV1::from_foundation(&fixture.foundation),
        Err(ObjectSymbolSurfaceBuildError::MissingPrimarySymbolRequest {
            definition_plan: fixture.plan.id(),
            symbol: PersistentSymbolKey::CallableBody(fixture.body.id()),
        })
    );
}

#[test]
fn rejects_two_odr_members_claiming_one_registration_symbol() {
    use scoop_identity::{
        CoreBuiltinNominal, ExactTypeKey, OdrGroupId, OdrMemberDiscriminator, OdrMemberKey,
        OdrMemberRole, PersistentExactTypeId, SpecializationKey,
    };
    let fixture = fixture(true);
    let mut members = Vec::new();
    let mut plans = vec![fixture.plan.clone()];
    let mut atoms = vec![fixture.primary.clone(), fixture.eh_frame.clone()];
    for builtin in [CoreBuiltinNominal::Any, CoreBuiltinNominal::Unit] {
        let exact =
            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(builtin.identity_record().id()))
                .unwrap();
        let group =
            OdrGroupId::from_key(&SpecializationKey::StructuralType { exact_type: exact }).unwrap();
        let member = CborIdentityRecord::from_key(
            OdrMemberKey::new(
                group,
                OdrMemberRole::RegistrationRecord,
                OdrMemberDiscriminator::CallableBody(fixture.body.id()),
            )
            .unwrap(),
        )
        .unwrap();
        let plan = CborIdentityRecord::from_key(ObjectDefinitionPlanKey::odr(member.id())).unwrap();
        atoms.push(atom(&plan, DefinitionAtomRole::Primary));
        plans.push(plan);
        members.push(member);
    }
    let mut canonical = fixture.foundation.as_canonical().clone();
    canonical.set_odr_members(members).unwrap();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(vec![
            PersistentSymbolRequest::new(
                PersistentSymbolKey::CallableBody(fixture.body.id()),
                LinkageClass::ConeStrong,
            )
            .unwrap(),
            PersistentSymbolRequest::new(
                PersistentSymbolKey::CallableRegistration(fixture.body.id()),
                LinkageClass::OdrWeak,
            )
            .unwrap(),
        ])
        .unwrap(),
    );
    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    assert!(matches!(
        ObjectSymbolSurfaceV1::from_foundation(&foundation),
        Err(ObjectSymbolSurfaceBuildError::DuplicatePrimarySymbol { .. })
    ));
}

struct Fixture {
    body: RuntimeIdentityRecord<PersistentCallableBodyId>,
    plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    eh_frame: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    foundation: ConeLirFoundation,
}

fn fixture(with_symbol: bool) -> Fixture {
    let function =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("entry").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(function.id()),
    ))
    .unwrap();
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let primary = atom(&plan, DefinitionAtomRole::Primary);
    let eh_frame = atom(&plan, DefinitionAtomRole::EhFrame);

    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_callable_bodies(vec![body.clone()]).unwrap();
    canonical.set_definition_plans(vec![plan.clone()]).unwrap();
    canonical
        .set_definition_atoms(vec![primary.clone(), eh_frame.clone()])
        .unwrap();
    if with_symbol {
        canonical.set_symbol_requests(
            PersistentSymbolRequestTable::new(vec![
                PersistentSymbolRequest::new(
                    PersistentSymbolKey::CallableBody(body.id()),
                    LinkageClass::ConeStrong,
                )
                .unwrap(),
            ])
            .unwrap(),
        );
    }
    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    Fixture {
        body,
        plan,
        primary,
        eh_frame,
        foundation,
    }
}

fn atom(
    plan: &CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    role: DefinitionAtomRole,
) -> CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        role,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}
