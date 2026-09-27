use super::*;
use crate::cross_cone_type_semantics::slot_contracts::tests::support::Fixture;
use crate::*;
use scoop_identity::{PendingIdentityValidation, SourceNominalKind};
use scoop_wire::{decode_canonical, encode};

#[test]
fn inheritance_wire_keeps_seven_fields_and_rejects_retired_fields() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Class);
    let record = NominalInheritanceInterfaceV1::try_new(
        fixture.inheritance.records[&owner.exact].clone(),
        CanonicalInheritanceSlotContractsV1::try_new(vec![]).unwrap(),
        CanonicalProtectedDeclarationRefsV1::default(),
        CanonicalInheritanceSlotSchemasV1::default(),
    )
    .unwrap();
    let bytes = encode(&record).unwrap();
    assert_eq!(bytes[0], 0xa7);
    let decoded: DecodedNominalInheritanceInterfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    for length in [0xa6, 0xa8] {
        let mut malformed = bytes.clone();
        malformed[0] = length;
        assert!(decode_canonical::<DecodedNominalInheritanceInterfaceV1>(&malformed).is_err());
    }
    for retired in [5, 6] {
        let mut malformed = bytes.clone();
        let index = malformed.len() - 6;
        assert_eq!(malformed[index], 7);
        malformed[index] = retired;
        assert!(decode_canonical::<DecodedNominalInheritanceInterfaceV1>(&malformed).is_err());
    }
    let mut absent = PendingIdentityValidation::new().finish().unwrap();
    assert!(decoded.resolve(&mut absent).is_err());
}

#[test]
fn inheritance_record_requires_the_complete_slot_schema_union() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Class);
    let slot = fixture.method(owner, "call", vec![]);
    fixture.schema(owner, &[slot]);
    let contract = fixture.contract(
        owner,
        slot,
        InheritanceSlotImplementationV1::Abstract(fixture.abstract_target(owner, slot)),
    );
    let schemas = fixture.schemas[&owner.exact].clone();
    for (contracts, schemas) in [
        (vec![], schemas),
        (vec![contract], CanonicalInheritanceSlotSchemasV1::default()),
    ] {
        assert_eq!(
            NominalInheritanceInterfaceV1::try_new(
                fixture.inheritance.records[&owner.exact].clone(),
                CanonicalInheritanceSlotContractsV1::try_new(contracts).unwrap(),
                CanonicalProtectedDeclarationRefsV1::default(),
                schemas,
            ),
            Err(InheritanceInterfaceBuildError::SlotClosure)
        );
    }
}

#[test]
fn protected_member_reference_rejects_retired_constructor_tag() {
    let bytes = [0xa2, 0, 2, 1, 0];
    assert!(matches!(
        decode_canonical::<DecodedProtectedDeclarationRefV1>(&bytes)
            .unwrap_err()
            .kind(),
        scoop_wire::WireErrorKind::UnknownTag { tag: 2 }
    ));
}
