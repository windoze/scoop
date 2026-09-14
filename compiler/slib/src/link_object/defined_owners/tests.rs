use scoop_identity::{ConeIdentity, StrongDefinitionEntity, StrongDefinitionRole};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::super::strong_relocation_closure::tests::verified_member_without_relocations;
use super::super::symbol_verification::tests::{fixture_for_producer, fixture_named};
use super::*;
use crate::verify_current_cone_strong_relocation_closure_v1;

#[test]
fn assigns_typed_primary_and_boundary_owners_to_the_actual_member() {
    let fixture = fixture_named("ownedDefinition");
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&fixture),
    ])
    .unwrap();

    let owners =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap();

    assert_eq!(owners.producer(), ConeIdentity::CORE);
    assert_eq!(owners.owners().len(), 3);
    assert!(
        owners
            .owners()
            .iter()
            .all(|owner| owner.member() == fixture.symbols.member())
    );
    assert!(owners.owners().iter().any(|owner| {
        owner.owner()
            == LinkDefinitionOwnerV1::StrongDefinition(
                StrongDefinitionOwnerV1::new(
                    StrongDefinitionEntity::callable_body(body(&fixture)),
                    StrongDefinitionRole::CallableBody,
                )
                .unwrap(),
            )
    }));
    assert!(owners.owners().iter().any(|owner| {
        owner.owner()
            == LinkDefinitionOwnerV1::VerifierBoundary {
                atom: fixture.atom,
                boundary: VerifiedBoundaryRoleV1::Start,
            }
    }));
    assert!(owners.owners().iter().any(|owner| {
        owner.owner()
            == LinkDefinitionOwnerV1::VerifierBoundary {
                atom: fixture.atom,
                boundary: VerifiedBoundaryRoleV1::End,
            }
    }));
}

#[test]
fn owner_set_is_canonical_across_member_input_order() {
    let first = fixture_for_producer(ConeIdentity::CORE, "firstOwner");
    let second = fixture_for_producer(ConeIdentity::CORE, "secondOwner");
    let forward = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&first),
        verified_member_without_relocations(&second),
    ])
    .unwrap();
    let reverse = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&second),
        verified_member_without_relocations(&first),
    ])
    .unwrap();

    assert_eq!(
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&forward).unwrap(),
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&reverse).unwrap()
    );
}

#[test]
fn owner_set_wire_round_trips_only_against_the_rebuilt_projection() {
    let first = fixture_for_producer(ConeIdentity::CORE, "wireFirst");
    let second = fixture_for_producer(ConeIdentity::CORE, "wireSecond");
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&second),
        verified_member_without_relocations(&first),
    ])
    .unwrap();
    let expected =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap();
    let bytes = encode(&expected).unwrap();
    let decoded: DecodedCanonicalDefinedLinkSymbolOwnerSetV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();

    assert_eq!(
        decoded.clone().validate_against(&expected),
        Ok(expected.clone())
    );

    let mut reordered = decoded;
    reordered.owners.swap(0, 1);
    assert_eq!(
        reordered.validate_against(&expected),
        Err(DefinedLinkSymbolOwnerValidationError::ProjectionMismatch)
    );
}

#[test]
fn owner_set_reader_rejects_non_closed_records() {
    for bytes in [vec![0x81, 0xa2], vec![0x81, 0xa4]] {
        assert!(
            decode_canonical::<DecodedCanonicalDefinedLinkSymbolOwnerSetV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }
}

fn body(
    fixture: &super::super::symbol_verification::tests::Fixture,
) -> scoop_identity::PersistentCallableBodyId {
    fixture
        .symbols
        .symbols()
        .iter()
        .find_map(|symbol| match symbol.role() {
            super::super::PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { owner, .. } => {
                match owner.kind() {
                    scoop_identity::StrongDefinitionEntityKind::CallableBody(body) => Some(body),
                    _ => None,
                }
            }
            _ => None,
        })
        .unwrap()
}
