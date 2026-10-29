use scoop_identity::{ConeIdentity, NativeLibraryBinding};
use scoop_lir::{CanonicalLirFoundation, ConeLirFoundation, GeneratedBridgePlanSetV1};
use scoop_wire::encode;

use super::super::current_cone_requirements::tests::primary_bridge_requirement;
use super::super::native_requirements::tests::{contract_record, native_surface};
use super::super::runtime_requirements::tests::classify;
use super::super::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use super::super::symbol_verification::tests::{fixture_for_producer, fixture_named};
use super::*;
use crate::{
    verify_current_cone_strong_relocation_closure_v1,
    verify_current_cone_undefined_requirements_v1, verify_dependency_strong_requirements_v1,
    verify_runtime_and_eh_requirements_v1, verify_source_external_requirements_v1,
};

#[test]
fn finalizes_runtime_and_eh_requirements_with_a_fixed_wire_vector() {
    let runtime = final_from_external(classify(b"_scoop_rt_allocation_context"));
    assert_eq!(runtime.requirements().len(), 1);
    assert!(matches!(
        runtime.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::RuntimeAbi { .. }
    ));
    assert_eq!(
        hex(&encode(&runtime).unwrap()),
        "81a201aa0158202e5d49dbf4da7d17d2e24f2667d42597f2ecba73145190c47cffcf99cadaeb480258209e52db568d91a313b2853c4eacd26323fc3a51d8a43764c34da23d4fb16941e5030104010500060407a10009081addccbbaa09010a581c5f73636f6f705f72745f616c6c6f636174696f6e5f636f6e7465787402a20005015820baaa52c37f0b2efd1cb708fd87c070b32fb53111b5796d59002803322fda46e1"
    );

    let eh = final_from_external(classify(b"__Unwind_Resume"));
    assert!(matches!(
        eh.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::TargetEhSupport { .. }
    ));
}

#[test]
fn native_requirement_records_the_symbol_bytes_with_a_new_tag() {
    let symbol = b"_memcmp".to_vec();
    let requirement = FinalUndefinedSymbolRequirementV1::Native {
        symbol: symbol.clone(),
    };
    let mut expected = vec![0xa2, 0, 10, 1, 0x47];
    expected.extend_from_slice(&symbol);
    assert_eq!(encode(&requirement).unwrap(), expected);
}

#[test]
fn finalizes_ordinary_native_symbols_without_source_contracts() {
    for symbol in [b"_memcmp".as_slice(), b"_toolchain_helper", b"_native_\xff"] {
        let final_set = final_from_external(classify(symbol));
        assert_eq!(final_set.requirements().len(), 1);
        assert_eq!(final_set.requirements()[0].use_site().symbol(), symbol);
        assert_eq!(
            final_set.requirements()[0].requirement(),
            FinalUndefinedSymbolRequirementV1::Native {
                symbol: symbol.to_vec()
            },
        );
    }
}

#[test]
fn finalizes_current_cone_and_generated_bridge_requirements() {
    let source = fixture_named("finalIntraSource");
    let target = fixture_named("finalIntraTarget");
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
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_with_undefined(&source, target_symbol.macho_name()),
        verified_member_without_relocations(&target),
    ])
    .unwrap();
    let current = verify_current_cone_undefined_requirements_v1(
        strong.clone(),
        empty_bridge_plan(ConeIdentity::CORE),
    )
    .unwrap();
    let final_set = finalize_undefined_symbol_requirements_v1(
        current,
        classify_without_source_externals(strong),
    );
    assert!(matches!(
        final_set.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::IntraConeStrong { .. }
    ));

    let (current, unit) = primary_bridge_requirement();
    let strong = current.strong_closure().clone();
    let final_set = finalize_undefined_symbol_requirements_v1(
        current,
        classify_without_source_externals(strong),
    );
    assert_eq!(
        final_set.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::GeneratedBridge { unit }
    );
}

#[test]
fn object_requirements_find_every_sorted_use_and_preserve_full_site_matching() {
    let fixtures = (0..16)
        .map(|index| fixture_named(&format!("indexedRequirement{index}")))
        .collect::<Vec<_>>();
    let members = fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| {
            verified_member_with_undefined(fixture, format!("_ordinary_native_{index}").as_bytes())
        })
        .collect();
    let strong = verify_current_cone_strong_relocation_closure_v1(members).unwrap();
    let current = verify_current_cone_undefined_requirements_v1(
        strong.clone(),
        empty_bridge_plan(ConeIdentity::CORE),
    )
    .unwrap();
    let finalized = finalize_undefined_symbol_requirements_v1(
        current,
        classify_without_source_externals(strong),
    );
    assert_eq!(finalized.requirements().len(), 16);
    let lookup = VerifiedObjectDefinitionRequirementSetV1::from(finalized.clone());
    for requirement in finalized.requirements() {
        assert_eq!(
            lookup.requirement_for(requirement.use_site()),
            Some(CanonicalObjectDefinitionRequirementV1::Legacy(
                requirement.requirement()
            ))
        );
    }
    for fixture in [&fixtures[0], &fixture_named("missingRequirement")] {
        let different =
            verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
                fixture,
                b"_different_native",
            )])
            .unwrap();
        let use_site = CanonicalUndefinedRelocationUseV1::from(&different.bindings()[0]);
        assert!(lookup.requirement_for(&use_site).is_none());
    }
}

#[test]
fn finalizes_source_external_requirements() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "finalNativeConsumer");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &source,
            b"_final_native",
        )])
        .unwrap();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(producer))
            .unwrap();
    let core =
        verify_dependency_strong_requirements_v1(scoop_target_data::DARWIN_AARCH64, strong, &[])
            .unwrap();
    let native = native_surface(
        producer,
        vec![contract_record(
            producer,
            "finalNativeDeclaration",
            "final_native",
            NativeLibraryBinding::DefaultNativeNamespace,
        )],
        Vec::new(),
    );
    let source = verify_source_external_requirements_v1(core, native).unwrap();
    let external =
        verify_runtime_and_eh_requirements_v1(source, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
            .unwrap();
    let final_set = finalize_undefined_symbol_requirements_v1(current, external);
    assert!(matches!(
        final_set.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::SourceExtern {
            library: NativeLibraryBinding::DefaultNativeNamespace,
            ..
        }
    ));
}

fn final_from_external(
    external: crate::VerifiedRuntimeAndEhRequirementClosureV1,
) -> CanonicalUndefinedSymbolRequirementSetV1 {
    let producer = external.producer();
    let strong = external
        .source_closure()
        .cross_cone_closure()
        .strong_closure()
        .clone();
    let current =
        verify_current_cone_undefined_requirements_v1(strong, empty_bridge_plan(producer)).unwrap();
    finalize_undefined_symbol_requirements_v1(current, external)
}

pub(in crate::link_object) fn classify_without_source_externals(
    strong: crate::VerifiedCurrentConeStrongRelocationClosureV1,
) -> VerifiedRuntimeAndEhRequirementClosureV1 {
    let producer = strong.producer();
    let core =
        verify_dependency_strong_requirements_v1(scoop_target_data::DARWIN_AARCH64, strong, &[])
            .unwrap();
    let source = verify_source_external_requirements_v1(
        core,
        native_surface(producer, Vec::new(), Vec::new()),
    )
    .unwrap();
    verify_runtime_and_eh_requirements_v1(source, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
        .unwrap()
}

pub(in crate::link_object) fn empty_final_requirements_for_strong(
    strong: crate::VerifiedCurrentConeStrongRelocationClosureV1,
) -> CanonicalUndefinedSymbolRequirementSetV1 {
    let producer = strong.producer();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(producer))
            .unwrap();
    let core =
        verify_dependency_strong_requirements_v1(scoop_target_data::DARWIN_AARCH64, strong, &[])
            .unwrap();
    let source = verify_source_external_requirements_v1(
        core,
        native_surface(producer, Vec::new(), Vec::new()),
    )
    .unwrap();
    let external =
        verify_runtime_and_eh_requirements_v1(source, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
            .unwrap();
    finalize_undefined_symbol_requirements_v1(current, external)
}

pub(in crate::link_object) fn empty_bridge_plan(
    producer: ConeIdentity,
) -> GeneratedBridgePlanSetV1 {
    let foundation = ConeLirFoundation::try_new(producer, CanonicalLirFoundation::empty()).unwrap();
    GeneratedBridgePlanSetV1::from_foundation(&foundation).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
