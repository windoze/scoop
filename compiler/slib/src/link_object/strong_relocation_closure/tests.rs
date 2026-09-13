use super::super::relocation_verification::tests::add_undefined_symbol;
use super::super::symbol_verification::tests::{
    Fixture, fixture_for_producer, fixture_named, object_for_plan_with_branch_relocation,
};
use super::*;
use crate::{
    PlannedStrongObjectSymbolRoleV1, validate_scoop_lir_llvm_22_1_object_envelope_v1,
    verify_member_object_relocations_v1, verify_member_strong_object_definitions_v1,
};

#[test]
fn resolves_cross_member_strong_uses_and_keeps_true_externals_explicit() {
    let source = fixture_named("source");
    let target = fixture_named("target");
    let target_primary_name = primary_name(&target).to_vec();
    let source_member = verified_member_with_undefined(&source, &target_primary_name);
    let target_member = verified_member_without_relocations(&target);

    let closure =
        verify_current_cone_strong_relocation_closure_v1(vec![target_member, source_member])
            .unwrap();

    assert_eq!(closure.members().len(), 2);
    assert_eq!(closure.bindings().len(), 1);
    let binding = &closure.bindings()[0];
    assert_eq!(binding.source_member(), source.symbols.member());
    assert_eq!(binding.containing_atom(), source.atom);
    assert_eq!(
        binding.containing_atom_role(),
        scoop_identity::DefinitionAtomRole::Primary
    );
    assert_eq!(binding.offset_within_atom(), 0);
    assert_eq!(
        binding.relocation_form(),
        VerifiedDarwinArm64RelocationFormV1::Branch26
    );
    assert_eq!(binding.encoded_value(), 0xddcc_bbaa);
    assert_eq!(binding.target_slot(), RelocationTargetSlotV1::Single);
    assert_eq!(binding.symbol(), target_primary_name);
    let PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
        owner,
        definition_role,
        ..
    } = primary_role(&target)
    else {
        unreachable!()
    };
    assert_eq!(
        binding.resolution(),
        StrongRelocationResolutionV1::CurrentConeStrong {
            target_member: target.symbols.member(),
            definition: target.plan,
            owner,
            definition_role,
        }
    );

    let external = fixture_named("externalSource");
    let external_member = verified_member_with_undefined(&external, b"_native_external");
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![external_member]).unwrap();
    assert_eq!(
        closure.bindings()[0].resolution(),
        StrongRelocationResolutionV1::ExternalCandidate {
            object_symbol_table_index: 3,
        }
    );
    assert_eq!(closure.bindings()[0].symbol(), b"_native_external");
}

#[test]
fn rejects_boundary_and_redundant_same_member_undefined_targets() {
    let source = fixture_named("boundarySource");
    let target = fixture_named("boundaryTarget");
    let boundary = target
        .symbols
        .symbols()
        .iter()
        .find(|symbol| {
            matches!(
                symbol.role(),
                PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. }
            )
        })
        .unwrap();
    let source_member = verified_member_with_undefined(&source, boundary.macho_name());
    let target_member = verified_member_without_relocations(&target);
    assert_eq!(
        verify_current_cone_strong_relocation_closure_v1(vec![source_member, target_member]),
        Err(
            StrongRelocationClosureValidationError::ExternalBoundaryTarget {
                atom: target.atom,
                boundary: crate::VerifiedBoundaryRoleV1::Start,
            }
        )
    );

    let source = fixture_named("redundantSource");
    let own_primary = primary_name(&source).to_vec();
    let member = verified_member_with_undefined(&source, &own_primary);
    assert_eq!(
        verify_current_cone_strong_relocation_closure_v1(vec![member]),
        Err(
            StrongRelocationClosureValidationError::RedundantLocalUndefined {
                member: source.symbols.member(),
                definition: source.plan,
                table_index: 3,
            }
        )
    );
}

#[test]
fn requires_a_unique_nonempty_member_set() {
    assert_eq!(
        verify_current_cone_strong_relocation_closure_v1(Vec::new()),
        Err(StrongRelocationClosureValidationError::NoLinkObjectMembers)
    );

    let fixture = fixture_named("duplicateMember");
    let member = verified_member_without_relocations(&fixture);
    assert_eq!(
        verify_current_cone_strong_relocation_closure_v1(vec![member.clone(), member]),
        Err(StrongRelocationClosureValidationError::DuplicateMember(
            fixture.symbols.member()
        ))
    );

    let other = fixture_for_producer(scoop_identity::ConeIdentity::SINGLE_FILE, "otherProducer");
    let other_member = verified_member_without_relocations(&other);
    assert!(matches!(
        verify_current_cone_strong_relocation_closure_v1(vec![
            verified_member_without_relocations(&fixture),
            other_member,
        ]),
        Err(StrongRelocationClosureValidationError::MixedProducer { .. })
    ));
}

pub(in crate::link_object) fn verified_member_with_undefined(
    fixture: &Fixture,
    undefined_name: &[u8],
) -> VerifiedMemberObjectRelocationIndexV1 {
    let object = object_for_plan_with_branch_relocation(
        &fixture.symbols,
        canonical_value,
        Some((0, primary_role(fixture))),
    );
    let bytes = add_undefined_symbol(object.bytes, true, undefined_name);
    verified_member(fixture, &bytes)
}

pub(in crate::link_object) fn verified_member_without_relocations(
    fixture: &Fixture,
) -> VerifiedMemberObjectRelocationIndexV1 {
    let object = object_for_plan_with_branch_relocation(&fixture.symbols, canonical_value, None);
    verified_member(fixture, &object.bytes)
}

fn verified_member(fixture: &Fixture, bytes: &[u8]) -> VerifiedMemberObjectRelocationIndexV1 {
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(bytes, sections, &fixture.symbols).unwrap();
    verify_member_object_relocations_v1(definitions).unwrap()
}

fn primary_role(fixture: &Fixture) -> PlannedStrongObjectSymbolRoleV1 {
    fixture
        .symbols
        .symbols()
        .iter()
        .find_map(|symbol| match symbol.role() {
            role @ PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. } => Some(role),
            _ => None,
        })
        .unwrap()
}

fn primary_name(fixture: &Fixture) -> &[u8] {
    fixture
        .symbols
        .symbols()
        .iter()
        .find(|symbol| {
            matches!(
                symbol.role(),
                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            )
        })
        .unwrap()
        .macho_name()
}

fn canonical_value(role: PlannedStrongObjectSymbolRoleV1) -> u64 {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 4,
    }
}
