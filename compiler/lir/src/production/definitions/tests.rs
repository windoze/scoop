use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DecodedCborIdentityRecord, DecodedSourceDeclarationKey, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, IdentityLayer, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PackagePath,
    PendingIdentityValidation, PersistentFunctionId, RuntimeIdentityRecord, SourceDeclarationKey,
    SourceDeclarationSite, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole, ValidatedIdentityGraph,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{CanonicalLirFoundation, ConeLirFoundation, DecodedLirFoundation};

#[test]
fn definition_plan_surface_has_a_fixed_wire_vector_and_validates() {
    let (surface, mut identities, foundation) = fixture();
    assert_eq!(
        hex(&encode(&surface).unwrap()),
        "81a30158203d751943715733e162e06681a6b144db51ce356b498c1964e7f6d0cceb68509602582098437a2d4cdd7cf6dce5fab22a24b7d7c1a913ef1ad362401ac3f40358b7a21603815820c59ceee916c019d90d7490fa770a23888f081460709b4b19f4f0a3601beed02b"
    );

    let decoded: DecodedStrongObjectDefinitionPlanSurfaceV1 =
        decode_canonical(&encode(&surface).unwrap()).unwrap();
    assert_eq!(decoded.validate(&mut identities, &foundation), Ok(surface));
}

#[test]
fn definition_plan_reader_rejects_non_closed_products() {
    for bytes in [vec![0x81, 0xa2], vec![0x81, 0xa4]] {
        assert!(decode_canonical::<DecodedStrongObjectDefinitionPlanSurfaceV1>(&bytes,).is_err());
    }
}

#[test]
fn producer_rejects_missing_multiple_and_orphan_primary_relations() {
    let records = records();

    let missing = foundation(&records, vec![associated_atom(&records)]);
    let missing = ConeLirFoundation::try_new(ConeIdentity::CORE, missing).unwrap();
    assert_eq!(
        StrongObjectDefinitionPlanSurfaceV1::from_foundation(&missing),
        Err(StrongObjectDefinitionPlanBuildError::MissingPrimaryAtom(
            records.plan.id()
        ))
    );

    let other_primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        records.plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::CallableBody(records.body.id()),
    ))
    .unwrap();
    let first_primary = primary_atom(&records);
    let expected_first = first_primary.id().min(other_primary.id());
    let expected_second = first_primary.id().max(other_primary.id());
    let multiple = foundation(&records, vec![first_primary, other_primary]);
    let multiple = ConeLirFoundation::try_new(ConeIdentity::CORE, multiple).unwrap();
    assert_eq!(
        StrongObjectDefinitionPlanSurfaceV1::from_foundation(&multiple),
        Err(StrongObjectDefinitionPlanBuildError::MultiplePrimaryAtoms {
            plan: records.plan.id(),
            first: expected_first,
            second: expected_second,
        })
    );

    let other_plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::callable_body(records.body.id()),
            StrongDefinitionRole::CallableRegistration,
        )
        .unwrap(),
    )
    .unwrap();
    let orphan = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        other_plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let orphan_id = orphan.id();
    let orphan_plan = other_plan.id();
    let invalid = foundation(&records, vec![primary_atom(&records), orphan]);
    let invalid = ConeLirFoundation::try_new(ConeIdentity::CORE, invalid).unwrap();
    assert_eq!(
        StrongObjectDefinitionPlanSurfaceV1::from_foundation(&invalid),
        Err(StrongObjectDefinitionPlanBuildError::OrphanAtom {
            atom: orphan_id,
            plan: orphan_plan,
        })
    );
}

#[test]
fn validation_rejects_noncanonical_and_semantically_mismatched_atoms() {
    let (surface, mut identities, foundation) = fixture();
    let bytes = encode(&surface).unwrap();
    let mut decoded: DecodedStrongObjectDefinitionPlanSurfaceV1 = decode_canonical(&bytes).unwrap();
    let duplicate = decoded.plans[0].associated_atoms[0];
    decoded.plans[0].associated_atoms.push(duplicate);
    assert!(matches!(
        decoded.validate(&mut identities, &foundation),
        Err(StrongObjectDefinitionPlanValidationError::DuplicateAssociatedAtom { .. })
    ));

    let mut decoded: DecodedStrongObjectDefinitionPlanSurfaceV1 = decode_canonical(&bytes).unwrap();
    let plan = &mut decoded.plans[0];
    std::mem::swap(&mut plan.primary_atom, &mut plan.associated_atoms[0]);
    assert_eq!(
        decoded.validate(&mut identities, &foundation),
        Err(StrongObjectDefinitionPlanValidationError::PlanMismatch { index: 0 })
    );
}

struct Records {
    function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    body: RuntimeIdentityRecord<scoop_identity::PersistentCallableBodyId>,
    plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
}

fn records() -> Records {
    let function = CborIdentityRecord::from_key(SourceDeclarationKey::function(
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
    Records {
        function,
        body,
        plan,
    }
}

fn primary_atom(
    records: &Records,
) -> CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        records.plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}

fn associated_atom(
    records: &Records,
) -> CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        records.plan.id(),
        DefinitionAtomRole::EhFrame,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}

fn foundation(
    records: &Records,
    atoms: Vec<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>,
) -> CanonicalLirFoundation {
    let mut foundation = CanonicalLirFoundation::empty();
    foundation
        .set_callable_bodies(vec![records.body.clone()])
        .unwrap();
    foundation
        .set_definition_plans(vec![records.plan.clone()])
        .unwrap();
    foundation.set_definition_atoms(atoms).unwrap();
    foundation
}

fn fixture() -> (
    StrongObjectDefinitionPlanSurfaceV1,
    ValidatedIdentityGraph,
    ConeLirFoundation,
) {
    let records = records();
    let canonical = foundation(
        &records,
        vec![primary_atom(&records), associated_atom(&records)],
    );
    let bytes = encode(&canonical).unwrap();
    let decoded: DecodedLirFoundation = decode_canonical(&bytes).unwrap();
    let decoded_function: DecodedCborIdentityRecord<
        PersistentFunctionId,
        DecodedSourceDeclarationKey,
    > = decode_canonical(&encode(&records.function).unwrap()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register(IdentityLayer::Hir, &decoded_function)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    pending.resolve(&decoded_function).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let mut identities = pending.finish().unwrap();
    let validated = decoded
        .validate(ConeIdentity::CORE, &mut identities)
        .unwrap();
    let foundation = ConeLirFoundation::from_validated(validated);
    let surface = StrongObjectDefinitionPlanSurfaceV1::from_foundation(&foundation).unwrap();
    (surface, identities, foundation)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
