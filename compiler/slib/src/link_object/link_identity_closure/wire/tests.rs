use scoop_identity::{DigestPatchIntentKey, DigestSemanticFieldRole};
use scoop_wire::{decode_canonical, encode};

use super::*;

mod digest_inputs;
mod materializations;
mod symbol_projections;
use crate::link_object::strong_relocation_closure::tests::verified_member_without_relocations;
use crate::link_object::symbol_verification::tests::fixture_named;
use crate::link_object::undefined_requirements::tests::empty_final_requirements_for_strong;
use crate::link_object::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalScoopLirObjectUnitSetV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, empty_code_link_object_member_set_for_test,
    verify_current_cone_strong_relocation_closure_v1,
};

#[test]
fn closure_wire_round_trips_only_against_the_rebuilt_projection() {
    let expected = closure();
    let bytes = encode(&expected).unwrap();
    assert_eq!(bytes[0], 0xa8);

    let decoded = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes).unwrap();
    assert_eq!(validate_against(decoded, &expected).unwrap(), expected);

    let decoded = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes).unwrap();
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
        assert!(decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes).is_err());
    }

    let expected = closure();
    let mut materialization = encode(&expected.materializations[0]).unwrap();
    assert_eq!(&materialization[..3], &[0xa3, 0x00, 0x01]);
    materialization[2] = 3;
    assert!(decode_canonical::<DecodedLinkObjectMaterializationV1>(&materialization).is_err());

    assert!(decode_canonical::<DecodedEntryOwnerBranchV1>(&[0xa1, 0x00, 0x03]).is_err());
}

#[test]
fn materialization_reader_rebuilds_the_member_plan_from_the_typed_partition() {
    let (partition, plan) = materialization_plan();
    let bytes = encoded_link_identity_closure_for_member_plan_test(&plan);
    let decoded = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes).unwrap();
    let checked = decoded
        .validate_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition)
        .unwrap();
    assert_eq!(checked.member_plan(), &plan);

    let decoded = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(
        &encoded_link_identity_closure_for_test(),
    )
    .unwrap();
    assert!(matches!(
        decoded.validate_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition),
        Err(LinkObjectMaterializationValidationError::UnknownScoopLirDefinition(_))
    ));

    let mut stale_member = closure();
    let stale_member_id = stale_member.materializations[0].member();
    stale_member.materializations = vec![super::super::LinkObjectMaterializationV1::ScoopLir {
        member: stale_member_id,
        units: plan.scoop_lir_members()[0].units().clone(),
    }];
    let decoded =
        decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&encode(&stale_member).unwrap())
            .unwrap();
    assert!(matches!(
        decoded.validate_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition),
        Err(LinkObjectMaterializationValidationError::ProjectionMismatch)
    ));
}

#[test]
fn patch_input_reader_matches_only_the_validated_digest_and_member_plans() {
    let coordinate = wire_fixture_coordinate();
    let producer = coordinate.identity().unwrap();
    let (canonical, production) =
        crate::link_decode::strong_production_fixture_for_test(coordinate, &[]);
    let foundation = scoop_lir::ConeLirFoundation::try_new(producer, canonical).unwrap();
    let partition = scoop_lir::ProducerUnitPartitionV1::from_foundation(&foundation).unwrap();
    let units =
        CanonicalScoopLirObjectUnitSetV1::new(partition.scoop_lir_definition_plans().to_vec())
            .unwrap();
    let plan = PlannedLinkObjectMemberSetV1::new(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        &partition,
        vec![units],
        Vec::new(),
    )
    .unwrap();
    let digest_plan = production.digest_finalization_plan();
    let intent = digest_plan.nodes()[0].patch_intents()[0].id();
    let member = plan
        .member_for_definition(
            digest_plan.nodes()[0].patch_intents()[0]
                .key()
                .target_definition(),
        )
        .unwrap();
    let bytes = encoded_link_identity_closure_for_patch_test(&plan, None, intent, member, 144);
    let decoded = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes).unwrap();
    let checked = decoded
        .validate_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition)
        .unwrap()
        .validate_digest_patch_inputs(digest_plan)
        .unwrap();
    assert_eq!(checked.member_plan(), &plan);
    assert_eq!(checked.provisional_patch_sites().len(), 1);
    assert_eq!(checked.provisional_patch_sites()[0].intent(), intent);
    assert_eq!(checked.provisional_patch_sites()[0].member(), member);
    assert_eq!(checked.provisional_patch_sites()[0].checked_offset(), 144);
    assert_eq!(checked.provisional_patch_sites()[0].width_bytes(), 32);

    let missing = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(
        &encoded_link_identity_closure_for_member_plan_test(&plan),
    )
    .unwrap()
    .validate_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition)
    .unwrap();
    assert_eq!(
        missing.validate_digest_patch_inputs(digest_plan),
        Err(LinkDigestPatchInputValidationError::MissingPatchIntent(
            intent
        ))
    );

    let expected_patch = &digest_plan.nodes()[0].patch_intents()[0];
    let unknown_intent = DigestPatchIntentId::from_key(&DigestPatchIntentKey::new(
        expected_patch.key().source(),
        expected_patch.key().target_definition(),
        expected_patch.key().atom_role(),
        DigestSemanticFieldRole::DescriptorDefinition,
    ))
    .unwrap();
    assert_ne!(unknown_intent, intent);
    let unknown = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(
        &encoded_link_identity_closure_for_patch_test(&plan, None, unknown_intent, member, 144),
    )
    .unwrap()
    .validate_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition)
    .unwrap();
    assert_eq!(
        unknown.validate_digest_patch_inputs(digest_plan),
        Err(LinkDigestPatchInputValidationError::UnknownPatchIntent(
            *unknown_intent.as_array()
        ))
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

pub(crate) fn encoded_link_identity_closure_for_test() -> Vec<u8> {
    encode(&closure()).unwrap()
}

pub(crate) fn encoded_link_identity_closure_for_member_plan_test(
    plan: &PlannedLinkObjectMemberSetV1,
) -> Vec<u8> {
    let mut projection = closure();
    projection.materializations = super::super::materializations(plan);
    encode(&projection).unwrap()
}

pub(crate) fn encoded_link_identity_closure_for_patch_test(
    plan: &PlannedLinkObjectMemberSetV1,
    builtins: Option<&VerifiedBuiltinObjectStrongRelocationSetV1>,
    intent: DigestPatchIntentId,
    member: SlibMemberId,
    checked_offset: u64,
) -> Vec<u8> {
    let mut projection = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(
        &encoded_link_identity_closure_for_member_plan_test(plan),
    )
    .unwrap();
    if let Some(builtins) = builtins {
        projection.definition_indexes = super::super::definition_indexes(builtins)
            .iter()
            .map(|index| decode_canonical(&encode(index).unwrap()).unwrap())
            .collect();
        let strong = builtins.strong_relocations().clone();
        let defined_symbols =
            CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong).unwrap();
        let undefined_symbols = empty_final_requirements_for_strong(strong);
        projection.defined_symbols = decode_canonical(&encode(&defined_symbols).unwrap()).unwrap();
        projection.undefined_symbols =
            decode_canonical(&encode(&undefined_symbols).unwrap()).unwrap();
    }
    projection.patch_sites = vec![DecodedMaterializedPatchSiteV1 {
        intent: decode_canonical(&encode(&intent).unwrap()).unwrap(),
        member: decode_canonical(&encode(&member).unwrap()).unwrap(),
        checked_offset,
    }];
    encode(&projection).unwrap()
}

pub(crate) fn encoded_link_identity_closure_without_symbol_projection_for_test(
    plan: &PlannedLinkObjectMemberSetV1,
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    intent: DigestPatchIntentId,
    member: SlibMemberId,
    checked_offset: u64,
) -> Vec<u8> {
    let mut projection = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(
        &encoded_link_identity_closure_for_patch_test(
            plan,
            Some(builtins),
            intent,
            member,
            checked_offset,
        ),
    )
    .unwrap();
    projection.defined_symbols =
        decode_canonical(&encode(closure().defined_symbols()).unwrap()).unwrap();
    encode(&projection).unwrap()
}

fn materialization_plan() -> (
    scoop_lir::ProducerUnitPartitionV1,
    PlannedLinkObjectMemberSetV1,
) {
    let coordinate = wire_fixture_coordinate();
    let producer = coordinate.identity().unwrap();
    let (canonical, _) = crate::link_decode::strong_production_fixture_for_test(coordinate, &[]);
    let foundation = scoop_lir::ConeLirFoundation::try_new(producer, canonical).unwrap();
    let partition = scoop_lir::ProducerUnitPartitionV1::from_foundation(&foundation).unwrap();
    let units =
        CanonicalScoopLirObjectUnitSetV1::new(partition.scoop_lir_definition_plans().to_vec())
            .unwrap();
    let plan = PlannedLinkObjectMemberSetV1::new(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        &partition,
        vec![units],
        Vec::new(),
    )
    .unwrap();
    (partition, plan)
}

fn wire_fixture_coordinate() -> scoop_identity::ConeCoordinate {
    scoop_identity::ConeCoordinate::new("test", "link-identity-closure-wire", "0.0.0").unwrap()
}
