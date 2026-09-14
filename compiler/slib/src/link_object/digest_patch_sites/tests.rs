use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentKey, DigestSemanticFieldRole,
};
use scoop_lir::{
    CBridgeProductionSetV1, CanonicalLirFoundation, DigestInputRefV1, GeneratedBridgePlanSetV1,
    LirTargetProfile, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongObjectSymbolSurfaceV1,
};

use super::*;
use crate::{
    PlannedStrongObjectSymbolRoleV1, PlannedStrongObjectSymbolSetV1,
    verify_builtin_object_strong_relocations_v1, verify_c_bridge_production_envelopes_v1,
};

use super::super::c_bridge_production::tests::{fixture, member_plan, profile};
use super::super::symbol_verification::tests::object_for_plan_with_deployment;

const MINIMUM_OS: u32 = 0x000d_0100;
const SDK: u32 = 0x000e_0200;
const SLOT_IN_ATOM: u64 = 16;

#[test]
fn verifies_complete_zero_patch_materialization() {
    let fixture = patch_fixture([0; 64], &[]);
    let site = ProvisionalDigestPatchSiteV1::new(
        fixture.intent,
        fixture.member,
        fixture.section_offset + SLOT_IN_ATOM,
        32,
    );

    let verified = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins,
        &fixture.foundation,
        fixture.digest_plan,
        &[ScoopLirObjectCandidateV1::new(
            fixture.member,
            &fixture.bytes,
        )],
        &[site],
    )
    .unwrap();

    assert_eq!(verified.sites().len(), 1);
    let actual = verified.sites()[0];
    assert_eq!(actual.intent(), fixture.intent);
    assert_eq!(actual.member(), fixture.member);
    assert_eq!(actual.definition(), fixture.definition);
    assert_eq!(actual.atom(), fixture.atom);
    assert_eq!(actual.atom_role(), DefinitionAtomRole::Primary);
    assert_eq!(actual.offset_within_atom(), SLOT_IN_ATOM);
    assert_eq!(actual.width_bytes(), 32);
}

#[test]
fn rejects_missing_wrong_width_and_out_of_atom_sites() {
    let fixture = patch_fixture([0; 64], &[]);
    let object = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.bytes,
    )];
    assert_eq!(
        verify_scoop_lir_digest_patch_sites_v1(
            fixture.builtins.clone(),
            &fixture.foundation,
            fixture.digest_plan.clone(),
            &object,
            &[],
        ),
        Err(DigestPatchSiteValidationError::MissingPatchIntent(
            fixture.intent
        ))
    );

    let wrong_width = ProvisionalDigestPatchSiteV1::new(
        fixture.intent,
        fixture.member,
        fixture.section_offset + SLOT_IN_ATOM,
        31,
    );
    assert_eq!(
        verify_scoop_lir_digest_patch_sites_v1(
            fixture.builtins.clone(),
            &fixture.foundation,
            fixture.digest_plan.clone(),
            &object,
            &[wrong_width],
        ),
        Err(DigestPatchSiteValidationError::PatchWidthMismatch {
            intent: fixture.intent,
            actual: 31,
        })
    );

    let outside = ProvisionalDigestPatchSiteV1::new(
        fixture.intent,
        fixture.member,
        fixture.section_offset + 40,
        32,
    );
    assert!(matches!(
        verify_scoop_lir_digest_patch_sites_v1(
            fixture.builtins,
            &fixture.foundation,
            fixture.digest_plan,
            &object,
            &[outside],
        ),
        Err(DigestPatchSiteValidationError::PatchOutsideAtom {
            intent,
            ..
        }) if intent == fixture.intent
    ));
}

#[test]
fn rejects_nonzero_or_relocated_slot_bytes() {
    let mut nonzero = [0; 64];
    nonzero[usize::try_from(SLOT_IN_ATOM).unwrap() + 7] = 1;
    let fixture = patch_fixture(nonzero, &[]);
    let site = patch_site(&fixture);
    assert_eq!(
        verify_scoop_lir_digest_patch_sites_v1(
            fixture.builtins,
            &fixture.foundation,
            fixture.digest_plan,
            &[ScoopLirObjectCandidateV1::new(
                fixture.member,
                &fixture.bytes,
            )],
            &[site],
        ),
        Err(DigestPatchSiteValidationError::NonZeroProvisionalSlot {
            intent: fixture.intent,
            byte_offset: fixture.section_offset + SLOT_IN_ATOM + 7,
        })
    );

    let relocated = patch_fixture([0; 64], &[u32::try_from(SLOT_IN_ATOM).unwrap()]);
    let site = patch_site(&relocated);
    assert_eq!(
        verify_scoop_lir_digest_patch_sites_v1(
            relocated.builtins,
            &relocated.foundation,
            relocated.digest_plan,
            &[ScoopLirObjectCandidateV1::new(
                relocated.member,
                &relocated.bytes,
            )],
            &[site],
        ),
        Err(DigestPatchSiteValidationError::RelocationOverlapsPatch {
            intent: relocated.intent,
            relocation_offset: SLOT_IN_ATOM,
        })
    );
}

#[test]
fn rechecks_the_exact_object_bytes_after_definition_verification() {
    let fixture = patch_fixture([0; 64], &[]);
    let site = patch_site(&fixture);
    let mut changed = fixture.bytes.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;

    assert_eq!(
        verify_scoop_lir_digest_patch_sites_v1(
            fixture.builtins,
            &fixture.foundation,
            fixture.digest_plan,
            &[ScoopLirObjectCandidateV1::new(fixture.member, &changed)],
            &[site],
        ),
        Err(DigestPatchSiteValidationError::ObjectBytesMismatch(
            fixture.member
        ))
    );
}

#[test]
fn rejects_a_digest_plan_validated_against_another_foundation() {
    let fixture = patch_fixture([0; 64], &[]);
    let unrelated = OdrFreeLirFoundation::try_new(
        fixture.foundation.producer(),
        CanonicalLirFoundation::empty(),
    )
    .unwrap();

    assert_eq!(
        verify_scoop_lir_digest_patch_sites_v1(
            fixture.builtins,
            &unrelated,
            fixture.digest_plan,
            &[],
            &[],
        ),
        Err(DigestPatchSiteValidationError::DigestPlan(
            scoop_lir::DigestPlanError::MissingPatchTarget(fixture.intent)
        ))
    );
}

#[test]
fn rejects_two_distinct_intents_materialized_over_the_same_bytes() {
    let mut fixture = patch_fixture([0; 64], &[]);
    let definition_key = DigestNodeKey::object_definition(fixture.atom);
    let definition_patch = DigestPatchIntentKey::new(
        DigestNodeId::from_key(&definition_key).unwrap(),
        fixture.definition,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::DescriptorDefinition,
    );
    let definition_node =
        scoop_lir::DigestNodeV1::new(definition_key, Vec::new(), vec![definition_patch]).unwrap();
    let image_key = DigestNodeKey::runtime_image(fixture.foundation.producer());
    let image_patch = DigestPatchIntentKey::new(
        DigestNodeId::from_key(&image_key).unwrap(),
        fixture.definition,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let image_node = scoop_lir::DigestNodeV1::new(
        image_key,
        vec![DigestInputRefV1::from_node(&definition_node)],
        vec![image_patch],
    )
    .unwrap();
    let mut intents = [
        definition_node.patch_intents()[0].id(),
        image_node.patch_intents()[0].id(),
    ];
    intents.sort_unstable();
    fixture.digest_plan =
        StrongDigestFinalizationPlanV1::new(vec![image_node, definition_node], &fixture.foundation)
            .unwrap();
    let sites = intents.map(|intent| {
        ProvisionalDigestPatchSiteV1::new(
            intent,
            fixture.member,
            fixture.section_offset + SLOT_IN_ATOM,
            32,
        )
    });

    assert_eq!(
        verify_scoop_lir_digest_patch_sites_v1(
            fixture.builtins,
            &fixture.foundation,
            fixture.digest_plan,
            &[ScoopLirObjectCandidateV1::new(
                fixture.member,
                &fixture.bytes,
            )],
            &sites,
        ),
        Err(DigestPatchSiteValidationError::OverlappingPatchSites {
            first: intents[0],
            second: intents[1],
        })
    );
}

fn patch_site(fixture: &PatchFixture) -> ProvisionalDigestPatchSiteV1 {
    ProvisionalDigestPatchSiteV1::new(
        fixture.intent,
        fixture.member,
        fixture.section_offset + SLOT_IN_ATOM,
        32,
    )
}

struct PatchFixture {
    foundation: scoop_lir::OdrFreeLirFoundation,
    digest_plan: StrongDigestFinalizationPlanV1,
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    bytes: Vec<u8>,
    section_offset: u64,
    member: SlibMemberId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    intent: DigestPatchIntentId,
}

fn patch_fixture(section_bytes: [u8; 64], relocation_offsets: &[u32]) -> PatchFixture {
    let base = fixture(None);
    let bridge_plan = GeneratedBridgePlanSetV1::from_odr_free_foundation(&base.foundation).unwrap();
    let member_plan = member_plan(&base, &bridge_plan);
    let surface = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&base.foundation).unwrap();
    let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
        LirTargetProfile::DARWIN_AARCH64,
        &surface,
        &member_plan,
    )
    .unwrap();
    let member = member_plan.scoop_lir_members()[0].member_id();
    let symbols = symbol_plan.member(member).unwrap();
    let primary = symbols
        .symbols()
        .iter()
        .find_map(|symbol| match symbol.role() {
            role @ PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. } => Some(role),
            _ => None,
        })
        .unwrap();
    let relocations = relocation_offsets
        .iter()
        .copied()
        .map(|offset| (offset, primary))
        .collect::<Vec<_>>();
    let object = object_for_plan_with_deployment(
        symbols,
        |role| match role {
            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 64,
        },
        &relocations,
        None,
        &section_bytes,
    );
    let scoop_objects = [ScoopLirObjectCandidateV1::new(member, &object.bytes)];
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
    let production = verify_c_bridge_production_envelopes_v1(
        bridge_plan.clone(),
        production,
        &profile,
        &member_plan,
        &[],
    )
    .unwrap();
    let builtins = verify_builtin_object_strong_relocations_v1(
        &member_plan,
        &symbol_plan,
        &scoop_objects,
        production,
        &[],
    )
    .unwrap();

    let key = DigestNodeKey::runtime_image(base.foundation.producer());
    let source = DigestNodeId::from_key(&key).unwrap();
    let patch = DigestPatchIntentKey::new(
        source,
        base.lir_plan,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let node = scoop_lir::DigestNodeV1::new(key, Vec::new(), vec![patch]).unwrap();
    let intent = node.patch_intents()[0].id();
    let digest_plan = StrongDigestFinalizationPlanV1::new(vec![node], &base.foundation).unwrap();
    let atom = base
        .foundation
        .resolve_definition_atom(base.lir_plan, DefinitionAtomRole::Primary)
        .unwrap()
        .1;
    PatchFixture {
        foundation: base.foundation,
        digest_plan,
        builtins,
        bytes: object.bytes,
        section_offset: u64::try_from(object.section_offset).unwrap(),
        member,
        definition: base.lir_plan,
        atom,
        intent,
    }
}
