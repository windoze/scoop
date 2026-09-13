use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, LinkageClass,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentCallableBodyId, PersistentFunctionId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    RuntimeIdentityRecord, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};

use super::*;
use crate::CanonicalLirFoundation;

#[test]
fn derives_primary_and_every_atom_boundary_from_typed_plans() {
    let fixture = fixture(true);
    let surface =
        StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&fixture.foundation).unwrap();
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
fn refuses_a_definition_without_its_foundation_primary_symbol() {
    let fixture = fixture(false);
    assert_eq!(
        StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&fixture.foundation),
        Err(
            StrongObjectSymbolSurfaceBuildError::MissingPrimarySymbolRequest {
                definition_plan: fixture.plan.id(),
                symbol: PersistentSymbolKey::CallableBody(fixture.body.id()),
            }
        )
    );
}

struct Fixture {
    body: RuntimeIdentityRecord<PersistentCallableBodyId>,
    plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    eh_frame: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    foundation: OdrFreeLirFoundation,
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
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
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
