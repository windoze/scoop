use scoop_identity::{
    CallbackParameterIndex, CborIdentityRecord, ConeIdentity, DefinitionAtomRole,
    DefinitionAtomSubkey, GeneratedBridgeAtomId, GeneratedBridgeAtomKey,
    GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitId, GeneratedBridgeUnitKey,
    NativeExternalContract, NativeExternalContractFingerprint, NativeExternalSymbolKey,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentNativeExternalSymbolId, SourceNativeSymbol, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_lir::{CanonicalLirFoundation, GeneratedBridgePlanSetV1, OdrFreeLirFoundation};

use super::*;
use crate::link_object::strong_relocation_closure::tests::{
    replace_only_binding_resolution, verified_member_with_local_primary_relocation,
    verified_member_with_undefined, verified_member_without_relocations,
};
use crate::link_object::symbol_verification::tests::fixture_named;
use crate::verify_current_cone_strong_relocation_closure_v1;

#[test]
fn classifies_only_cross_member_normal_strong_uses() {
    let source = fixture_named("requirementSource");
    let target = fixture_named("requirementTarget");
    let target_symbol = target
        .symbols
        .symbols()
        .iter()
        .find(|symbol| {
            matches!(
                symbol.role(),
                crate::PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            )
        })
        .unwrap();
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_with_undefined(&source, target_symbol.macho_name()),
        verified_member_without_relocations(&target),
    ])
    .unwrap();
    let plan = empty_bridge_plan(ConeIdentity::CORE);
    let verified = verify_current_cone_undefined_requirements_v1(closure, plan).unwrap();

    assert_eq!(verified.requirements().len(), 1);
    let requirement = &verified.requirements()[0];
    assert_eq!(
        requirement.use_site().source_member(),
        source.symbols.member()
    );
    assert_eq!(requirement.target_member(), target.symbols.member());
    assert_eq!(requirement.target_definition(), target.plan);
    assert!(matches!(
        requirement.requirement(),
        CurrentConeUndefinedRequirementV1::IntraConeStrong { .. }
    ));

    let local = fixture_named("localRequirementTarget");
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_with_local_primary_relocation(&local),
    ])
    .unwrap();
    let verified = verify_current_cone_undefined_requirements_v1(
        closure,
        empty_bridge_plan(ConeIdentity::CORE),
    )
    .unwrap();
    assert!(verified.requirements().is_empty());
}

#[test]
fn maps_only_a_planned_primary_bridge_atom_to_its_unit() {
    let bridge = bridge_plan(ConeIdentity::CORE);
    let closure =
        synthetic_cross_member_closure(LinkDefinitionOwnerV1::GeneratedBridge(bridge.primary));
    let verified =
        verify_current_cone_undefined_requirements_v1(closure, bridge.plan.clone()).unwrap();
    assert_eq!(
        verified.requirements()[0].requirement(),
        CurrentConeUndefinedRequirementV1::GeneratedBridge { unit: bridge.unit }
    );

    let closure =
        synthetic_cross_member_closure(LinkDefinitionOwnerV1::GeneratedBridge(bridge.associated));
    assert_eq!(
        verify_current_cone_undefined_requirements_v1(closure, bridge.plan),
        Err(
            CurrentConeUndefinedRequirementValidationError::NonPrimaryGeneratedBridgeTarget {
                atom: bridge.associated,
                unit: bridge.unit,
            }
        )
    );
}

#[test]
fn requires_the_bridge_plan_to_belong_to_the_object_producer() {
    let source = fixture_named("producerMismatch");
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&source),
    ])
    .unwrap();
    assert_eq!(
        verify_current_cone_undefined_requirements_v1(
            closure,
            empty_bridge_plan(ConeIdentity::SINGLE_FILE),
        ),
        Err(
            CurrentConeUndefinedRequirementValidationError::ProducerMismatch {
                object: ConeIdentity::CORE,
                bridge: ConeIdentity::SINGLE_FILE,
            }
        )
    );
}

fn synthetic_cross_member_closure(
    owner: LinkDefinitionOwnerV1,
) -> VerifiedCurrentConeStrongRelocationClosureV1 {
    let source = fixture_named("bridgeRequirementSource");
    let target = fixture_named("bridgeRequirementTarget");
    let target_symbol = target
        .symbols
        .symbols()
        .iter()
        .find(|symbol| {
            matches!(
                symbol.role(),
                crate::PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            )
        })
        .unwrap();
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_with_undefined(&source, target_symbol.macho_name()),
        verified_member_without_relocations(&target),
    ])
    .unwrap();
    replace_only_binding_resolution(
        closure,
        StrongRelocationResolutionV1::CurrentConeUndefinedStrong {
            target_member: target.symbols.member(),
            definition: target.plan,
            owner,
        },
    )
}

fn empty_bridge_plan(producer: ConeIdentity) -> GeneratedBridgePlanSetV1 {
    let foundation =
        OdrFreeLirFoundation::try_new(producer, CanonicalLirFoundation::empty()).unwrap();
    GeneratedBridgePlanSetV1::from_odr_free_foundation(&foundation).unwrap()
}

struct BridgePlanFixture {
    plan: GeneratedBridgePlanSetV1,
    unit: GeneratedBridgeUnitId,
    primary: GeneratedBridgeAtomId,
    associated: GeneratedBridgeAtomId,
}

fn bridge_plan(producer: ConeIdentity) -> BridgePlanFixture {
    let symbol = NativeExternalSymbolKey::darwin_macho_external(
        &SourceNativeSymbol::new("bridge_requirement").unwrap(),
    )
    .unwrap();
    let contract = NativeExternalContract::c_function(
        scoop_identity::NativeLibraryBinding::DefaultNativeNamespace,
        scoop_identity::CanonicalCAbiFunctionSignature::cdecl(
            Vec::new(),
            scoop_identity::CanonicalCAbiReturn::Void,
        ),
    );
    let fingerprint = NativeExternalContractFingerprint::from_symbol_and_contract(
        PersistentNativeExternalSymbolId::from_key(&symbol).unwrap(),
        &contract,
    )
    .unwrap();
    let unit = CborIdentityRecord::from_key(GeneratedBridgeUnitKey::OutboundFunction(fingerprint))
        .unwrap();
    let primary = CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
        producer,
        GeneratedBridgeAtomRoleKey::PrimaryEntry { unit: unit.id() },
    ))
    .unwrap();
    let associated = CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
        producer,
        GeneratedBridgeAtomRoleKey::ContextDescriptor {
            unit: unit.id(),
            context_index: CallbackParameterIndex::new(0),
        },
    ))
    .unwrap();
    let definition_plans = [
        generated_bridge_definition(producer, primary.key()),
        generated_bridge_definition(producer, associated.key()),
    ];
    let definition_atoms = definition_plans
        .iter()
        .map(|plan| {
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                plan.id(),
                DefinitionAtomRole::Primary,
                DefinitionAtomSubkey::Singleton,
            ))
            .unwrap()
        })
        .collect();
    let mut foundation = CanonicalLirFoundation::empty();
    foundation.set_bridge_units(vec![unit.clone()]).unwrap();
    foundation
        .set_bridge_atoms(vec![primary.clone(), associated.clone()])
        .unwrap();
    foundation
        .set_definition_plans(definition_plans.into_iter().collect())
        .unwrap();
    foundation.set_definition_atoms(definition_atoms).unwrap();
    let foundation = OdrFreeLirFoundation::try_new(producer, foundation).unwrap();

    BridgePlanFixture {
        plan: GeneratedBridgePlanSetV1::from_odr_free_foundation(&foundation).unwrap(),
        unit: unit.id(),
        primary: primary.id(),
        associated: associated.id(),
    }
}

fn generated_bridge_definition(
    producer: ConeIdentity,
    atom: &GeneratedBridgeAtomKey,
) -> CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey> {
    CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::generated_bridge_atom(atom).unwrap(),
            StrongDefinitionRole::GeneratedBridge,
        )
        .unwrap(),
    )
    .unwrap()
}
