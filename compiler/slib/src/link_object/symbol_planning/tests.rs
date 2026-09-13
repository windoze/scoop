use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, LinkageClass,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentCallableBodyId, PersistentFunctionId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    RuntimeIdentityRecord, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalLirFoundation, LirTargetProfile, OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1,
    StrongProducerUnitPartitionV1,
};

use super::*;
use crate::CanonicalScoopLirObjectUnitSetV1;

#[test]
fn assigns_primary_and_boundaries_to_the_definition_member() {
    let fixture = fixture("entry");
    let planned = PlannedStrongObjectSymbolSetV1::new(
        LirTargetProfile::DARWIN_AARCH64,
        &fixture.surface,
        &fixture.members,
    )
    .unwrap();
    assert_eq!(planned.members().len(), 1);
    let member = planned.member(fixture.member).unwrap();
    assert_eq!(member.symbols().len(), 3);

    let expected = [
        (
            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                definition: fixture.plan,
                primary_atom: fixture.atom,
            },
            PersistentSymbolKey::CallableBody(fixture.body),
        ),
        (
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
                definition: fixture.plan,
                atom: fixture.atom,
            },
            PersistentSymbolKey::DefinitionBoundaryStart(fixture.atom),
        ),
        (
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
                definition: fixture.plan,
                atom: fixture.atom,
            },
            PersistentSymbolKey::DefinitionBoundaryEnd(fixture.atom),
        ),
    ];
    for (role, key) in expected {
        let symbol = member
            .symbols()
            .iter()
            .find(|symbol| symbol.role() == role)
            .unwrap();
        assert_eq!(symbol.request().key(), key);
        assert_eq!(symbol.request().linkage(), LinkageClass::ConeStrong);
        assert_eq!(
            symbol.macho_name(),
            format!("_{}", symbol.request().symbol()).as_bytes()
        );
    }
}

#[test]
fn rejects_surfaces_and_member_assignments_from_different_plan_sets() {
    let left = fixture("left");
    let right = fixture("right");
    assert_eq!(
        PlannedStrongObjectSymbolSetV1::new(
            LirTargetProfile::DARWIN_AARCH64,
            &left.surface,
            &right.members,
        ),
        Err(StrongObjectSymbolPlanningError::MissingMemberAssignment(
            left.plan
        ))
    );
}

struct Fixture {
    body: PersistentCallableBodyId,
    plan: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    member: SlibMemberId,
    surface: StrongObjectSymbolSurfaceV1,
    members: PlannedLinkObjectMemberSetV1,
}

fn fixture(name: &str) -> Fixture {
    let function =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
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
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body.id()),
        LinkageClass::ConeStrong,
    )
    .unwrap();

    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_callable_bodies(vec![body.clone()]).unwrap();
    canonical.set_definition_plans(vec![plan.clone()]).unwrap();
    canonical.set_definition_atoms(vec![atom.clone()]).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    let surface = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    let partition = StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let members = PlannedLinkObjectMemberSetV1::new(
        ConeIdentity::CORE,
        &partition,
        vec![CanonicalScoopLirObjectUnitSetV1::new(vec![plan.id()]).unwrap()],
        Vec::new(),
    )
    .unwrap();
    let member = members.scoop_lir_members()[0].member_id();
    Fixture {
        body: body.id(),
        plan: plan.id(),
        atom: atom.id(),
        member,
        surface,
        members,
    }
}
