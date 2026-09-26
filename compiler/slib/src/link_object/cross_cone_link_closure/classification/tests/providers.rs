use super::*;
use scoop_lir::{CanonicalLirFoundation, CrossConeLirBridgeSectionV1, OdrFreeLirFoundation};

fn provider(name: &str) -> ConeIdentity {
    scoop_identity::ConeCoordinate::new("test", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

fn ordinary(
    consumer: ConeIdentity,
    callable: StrongExternalLirBridgeV1,
) -> CrossConeLirBridgeSectionV1 {
    let StrongExternalLirBridgeV1::Callable(callable) = callable;
    let foundation =
        OdrFreeLirFoundation::try_new(consumer, CanonicalLirFoundation::empty()).unwrap();
    CrossConeLirBridgeSectionV1::try_new(&foundation, vec![], vec![*callable]).unwrap()
}

#[test]
fn one_pass_resolves_mixed_providers_for_a_core_consumer() {
    let consumer = ConeIdentity::CORE;
    let service_provider = provider("service-provider");
    let ordinary_provider = provider("ordinary-provider");
    let service = callable_bridge(service_provider, "initialize");
    let callable = callable_bridge(ordinary_provider, "entry");
    let symbols = [
        bridge_name(&service),
        bridge_name(&callable),
        b"_native_unmatched".to_vec(),
    ];
    let members = symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            let object = fixture_for_producer(consumer, &format!("mixedUse{index}"));
            verified_member_with_undefined(&object, symbol)
        })
        .collect();
    let strong = verify_current_cone_strong_relocation_closure_v1(members).unwrap();
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(consumer, vec![service]).unwrap();
    let ordinary = ordinary(consumer, callable);
    let mut owners = vec![
        callable_owners(service_provider, "initialize"),
        callable_owners(ordinary_provider, "entry"),
    ];
    let run = |owners: &[CanonicalDefinedLinkSymbolOwnerSetV1]| {
        verify_cross_cone_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong.clone(),
            surface.clone(),
            owners,
            &ordinary,
        )
        .unwrap()
    };
    let result = run(&owners);
    owners.reverse();
    let reordered = run(&owners);
    assert_eq!(
        result.external_requirements(),
        reordered.external_requirements()
    );
    assert_eq!(result.requirements(), reordered.requirements());
    assert_eq!(result.external_requirements().len(), 1);
    assert_eq!(
        result
            .external_requirements()
            .iter()
            .filter(|use_| use_.provider() == service_provider)
            .count(),
        1
    );
    assert_eq!(
        result.semantic_imports().imports()[0].provider(),
        ordinary_provider
    );
    assert_eq!(result.requirements().len(), 1);
    assert_eq!(result.remaining_external_candidates().len(), 1);
    assert_eq!(
        result.remaining_external_candidates()[0].symbol(),
        b"_native_unmatched"
    );
}

#[test]
fn provider_lookup_rejects_missing_duplicate_self_and_wrong_owner_inputs() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let origin = provider("lookup-provider");
    let bridge = callable_bridge(origin, "entry");
    let name = bridge_name(&bridge);
    let object = fixture_for_producer(consumer, "lookupUse");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &object, &name,
        )])
        .unwrap();
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(consumer, vec![bridge]).unwrap();
    let owners = callable_owners(origin, "entry");
    let run = |owners: &[CanonicalDefinedLinkSymbolOwnerSetV1]| {
        verify_dependency_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong.clone(),
            surface.clone(),
            owners,
        )
    };
    assert!(
        matches!(run(&[]), Err(CrossConeStrongRequirementValidationError::MissingProvider { provider }) if provider == origin)
    );
    assert!(
        matches!(run(&[owners.clone(), owners.clone()]), Err(CrossConeStrongRequirementValidationError::DuplicateProvider { provider }) if provider == origin)
    );
    assert!(
        matches!(run(&[callable_owners(consumer, "self")]), Err(CrossConeStrongRequirementValidationError::SelfDependency { provider }) if provider == consumer)
    );
    let different = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::exact_type(exact_type(origin, "Different")),
        StrongDefinitionRole::TypeDescriptor,
    )
    .unwrap();
    let changed =
        owners.replace_owner_for_test(&name, LinkDefinitionOwnerV1::StrongDefinition(different));
    assert!(
        matches!(run(&[changed]), Err(CrossConeStrongRequirementValidationError::DependencyOwnerMismatch { provider, .. }) if provider == origin)
    );
    let other_provider = provider("other-lookup-provider");
    assert!(
        matches!(run(&[callable_owners(other_provider, "entry")]), Err(CrossConeStrongRequirementValidationError::MissingProvider { provider }) if provider == origin)
    );
}

#[test]
fn a_callable_cannot_be_claimed_by_two_capability_subjects() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let origin = provider("overlap-provider");
    let callable = callable_bridge(origin, "entry");
    let object = fixture_for_producer(consumer, "overlapUse");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &object,
            &bridge_name(&callable),
        )])
        .unwrap();
    let surface =
        StrongExternalLirBridgeSurfaceV1::try_new(consumer, vec![callable.clone()]).unwrap();
    let ordinary = ordinary(consumer, callable);
    assert!(matches!(
        verify_cross_cone_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            &[callable_owners(origin, "entry")],
            &ordinary,
        ),
        Err(CrossConeStrongRequirementValidationError::DuplicateNormalizedSymbol { .. })
    ));
}

#[test]
fn final_requirement_keeps_the_selected_callable_provider() {
    use crate::link_object::native_requirements::tests::native_surface;
    use crate::link_object::undefined_requirements::tests::empty_bridge_plan;
    use crate::link_object::*;

    let consumer = ConeIdentity::SINGLE_FILE;
    let origin = provider("final-provider");
    let bridge = callable_bridge(origin, "entry");
    let object = fixture_for_producer(consumer, "finalUse");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &object,
            &bridge_name(&bridge),
        )])
        .unwrap();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(consumer))
            .unwrap();
    let dependencies = verify_dependency_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        StrongExternalLirBridgeSurfaceV1::try_new(consumer, vec![bridge]).unwrap(),
        &[callable_owners(origin, "entry")],
    )
    .unwrap();
    let source = verify_source_external_requirements_v1(
        dependencies,
        native_surface(consumer, vec![], vec![]),
    )
    .unwrap();
    let runtime = verify_runtime_and_eh_requirements_v1(
        source,
        scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let external = seal_builtin_object_external_requirements_v1(
        without_generated_bridge_semantics_for_test(runtime),
    )
    .unwrap();
    let final_set = finalize_undefined_symbol_requirements_v1(current, external).unwrap();
    assert_eq!(final_set.requirements().len(), 1);
    assert!(
        matches!(final_set.requirements()[0].requirement(), FinalUndefinedSymbolRequirementV1::DependencyStrong { provider, owner }
        if provider == origin && owner.role() == StrongDefinitionRole::CallableBody)
    );
}
