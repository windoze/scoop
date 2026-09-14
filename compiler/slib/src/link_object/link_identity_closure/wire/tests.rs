use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::link_object::strong_relocation_closure::tests::verified_member_without_relocations;
use crate::link_object::symbol_verification::tests::fixture_named;
use crate::link_object::undefined_requirements::tests::empty_final_requirements_for_strong;
use crate::link_object::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalScoopLirObjectUnitSetV1,
    empty_code_link_object_member_set_for_test, verify_current_cone_strong_relocation_closure_v1,
};

#[test]
fn closure_wire_round_trips_only_against_the_rebuilt_projection() {
    let expected = closure();
    let bytes = encode(&expected).unwrap();
    assert_eq!(bytes[0], 0xa8);

    let decoded =
        decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(validate_against(decoded, &expected).unwrap(), expected);

    let decoded =
        decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    let mut changed = expected.clone();
    changed.image_owner.checked_offset += 1;
    assert!(matches!(
        validate_against(decoded, &changed),
        Err(LinkIdentityClosureSectionValidationError::ProjectionMismatch)
    ));
}

#[test]
fn closure_reader_rejects_old_extended_and_unknown_sum_shapes() {
    for bytes in [vec![0xa7], vec![0xa9]] {
        assert!(
            decode_canonical::<DecodedLinkIdentityClosureSectionV1>(
                &bytes,
                DecodeLimits::default()
            )
            .is_err()
        );
    }

    let expected = closure();
    let mut materialization = encode(&expected.materializations[0]).unwrap();
    assert_eq!(&materialization[..3], &[0xa3, 0x00, 0x01]);
    materialization[2] = 3;
    assert!(
        decode_canonical::<DecodedLinkObjectMaterializationV1>(
            &materialization,
            DecodeLimits::default()
        )
        .is_err()
    );

    assert!(
        decode_canonical::<DecodedEntryOwnerBranchV1>(&[0xa1, 0x00, 0x03], DecodeLimits::default())
            .is_err()
    );
}

fn closure() -> LinkIdentityClosureSectionV1 {
    let fixture = fixture_named("closureWireProjection");
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&fixture),
    ])
    .unwrap();
    let member = &strong.members()[0];
    let definition = &member.definitions().definitions()[0];
    let atom = definition.atoms()[0];
    let member_id = member.member();
    let definition_id = definition.definition();
    let primary_atom = definition.primary_atom();
    let primary_symbol_table_index = definition.primary_symbol_table_index();
    let defined_symbols =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong).unwrap();
    let undefined_symbols = empty_final_requirements_for_strong(strong);

    LinkIdentityClosureSectionV1 {
        materializations: vec![super::super::LinkObjectMaterializationV1::ScoopLir {
            member: member_id,
            units: CanonicalScoopLirObjectUnitSetV1::new(vec![definition_id]).unwrap(),
        }],
        definition_indexes: vec![super::super::VerifiedObjectDefinitionIndexV1 {
            member: member_id,
            definition: definition_id,
            primary_atom,
            primary_symbol_table_index,
            atoms: vec![super::super::VerifiedDefinitionAtomRangeProjectionV1 {
                atom: atom.atom(),
                atom_role: atom.atom_role(),
                section_ordinal: atom.section_ordinal().get(),
                start: atom.start(),
                end: atom.end(),
                padding_end: atom.padding_end(),
            }],
        }],
        patch_sites: Vec::new(),
        defined_symbols,
        undefined_symbols,
        verified_link_objects: empty_code_link_object_member_set_for_test(),
        image_owner: super::super::VerifiedImageOwnerProjectionV1 {
            member: member_id,
            definition: definition_id,
            primary_atom,
            primary_symbol_table_index,
            checked_offset: atom.start(),
            byte_size: atom.end() - atom.start(),
        },
        entry_owner: super::super::VerifiedEntryOwnerBranchV1::Library,
    }
}
