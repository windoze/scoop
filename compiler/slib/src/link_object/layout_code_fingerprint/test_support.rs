use scoop_identity::ConeCoordinate;
use scoop_lir::{
    CanonicalNativeExternalRequirementSurfaceV1, ConeLirFoundation, ConeProductionSectionV2,
    CrossConeLayoutAbiSectionV1, CrossConeLirBridgeSectionV1, EntryProductionSourceV1,
    StrongRegistrationProductionSurfaceV2,
};

use super::*;
use crate::link_object::layout_link_closure::tests::fixture::empty_section;
use crate::link_object::native_requirements::tests::dependency_closure;
use crate::link_object::strong_relocation_closure::tests::verified_member_without_relocations;
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::link_object::undefined_requirements::tests::empty_bridge_plan;
use crate::{
    ConeKind, ConeRecord, ConeSourceForm, DependencyRecord, HirFingerprint, LirFingerprint,
    MirFingerprint, SlibMember, finalize_layout_partitioned_undefined_symbol_requirements_v1,
    seal_builtin_object_external_requirements_v1, verify_current_cone_undefined_requirements_v1,
    verify_dependency_strong_requirements_v1, verify_external_shape_requirements_v1,
    verify_runtime_and_eh_requirements_v1,
    verify_source_external_requirements_after_external_shape_v1,
};

pub(crate) struct EmptyLayoutCodeFixture<'a> {
    pub cone: ConeRecord,
    pub dependencies: Vec<DependencyRecord>,
    pub foundation: &'a ConeLirFoundation,
    pub ordinary: &'a CrossConeLirBridgeSectionV1,
    pub layout: &'a CrossConeLayoutAbiSectionV1<'a>,
    pub code: CrossConeLayoutCodeDataV1,
    pub link_objects: Vec<SlibMember>,
}

pub(crate) fn with_empty_layout_code_fixture<R>(
    use_fixture: impl FnOnce(EmptyLayoutCodeFixture<'_>) -> R,
) -> R {
    let cone = cone();
    let producer = cone.identity();
    let objects =
        crate::link_decode::tests::layout_link_support::verified_layout_code_link_object_members();
    let member_plan = &objects
        .final_objects()
        .entry()
        .patch_sites()
        .builtins()
        .member_plan()
        .scoop_lir_members()[0];
    let link_member = SlibMember::new(
        producer,
        member_plan.stable_key().clone(),
        member_plan.role().clone(),
        objects.final_objects().objects()[0].bytes().to_vec(),
    )
    .unwrap();
    let (canonical, v1) = crate::link_decode::tests::strong_production_fixture(
        cone.coordinate().clone(),
        &[scoop_identity::ConeIdentity::CORE],
    );
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();
    let ordinary = CrossConeLirBridgeSectionV1::try_new(&foundation, vec![], vec![]).unwrap();
    let target = scoop_target_data::DARWIN_AARCH64;
    let layout = empty_section(producer);
    let v2 = ConeProductionSectionV2::new(
        cone.coordinate().clone(),
        v1.image_plan().dependencies(),
        &foundation,
        v1.digest_finalization_plan().clone(),
        StrongRegistrationProductionSurfaceV2::empty(
            target,
            &foundation,
            v1.digest_finalization_plan(),
        )
        .unwrap(),
        EntryProductionSourceV1::Library,
        &[],
        v1.canonical_callable_definitions().clone(),
        scoop_lir::CanonicalShapeAbisV1::new(Vec::new(), &foundation).unwrap(),
    )
    .unwrap()
    .validate_layout_abi(&layout)
    .unwrap();
    let dependencies = vec![
        DependencyRecord::new(
            ConeCoordinate::reserved_core(),
            HirFingerprint::from_array([1; 32]),
            MirFingerprint::from_array([2; 32]),
            LirFingerprint::from_array([3; 32]),
        )
        .unwrap(),
    ];
    let production = crate::verify_cross_cone_layout_production_code_projection_v1(
        &cone,
        &dependencies,
        0,
        v2,
        objects,
    )
    .unwrap();
    let strong = production
        .link_objects()
        .final_objects()
        .entry()
        .patch_sites()
        .builtins()
        .strong_relocations()
        .clone();
    let defined =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong).unwrap();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(producer))
            .unwrap();
    let callable = legacy_closure(strong);
    let shape = verify_external_shape_requirements_v1(
        &callable,
        layout.selected().consumer(),
        layout.selected().physical_imports(),
    )
    .unwrap();
    let native =
        CanonicalNativeExternalRequirementSurfaceV1::from_foundation(target, &foundation).unwrap();
    let source =
        verify_source_external_requirements_after_external_shape_v1(&shape, native.clone())
            .unwrap();
    let runtime =
        verify_runtime_and_eh_requirements_v1(source, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
            .unwrap();
    let external = seal_builtin_object_external_requirements_v1(
        crate::link_object::without_generated_bridge_semantics_for_test(runtime),
    )
    .unwrap();
    let partitions =
        finalize_layout_partitioned_undefined_symbol_requirements_v1(current, external, &shape)
            .unwrap();
    let code = compute_cross_cone_layout_code_fingerprint_v1(
        production,
        native,
        defined,
        partitions,
        &shape,
        crate::LirLinkSupportSectionV1::default(),
    )
    .unwrap();

    use_fixture(EmptyLayoutCodeFixture {
        cone,
        dependencies,
        foundation: &foundation,
        ordinary: &ordinary,
        layout: &layout,
        code,
        link_objects: vec![link_member],
    })
}

fn legacy_closure(
    strong: crate::VerifiedCurrentConeStrongRelocationClosureV1,
) -> crate::VerifiedCrossConeStrongRequirementClosureV1 {
    let producer = strong.producer();
    let object = fixture_for_producer(producer, "layoutWriterCoreOwner");
    let seed = dependency_closure(verified_member_without_relocations(&object));
    verify_dependency_strong_requirements_v1(
        scoop_target_data::DARWIN_AARCH64,
        strong,
        seed.dependency_owners(),
    )
    .unwrap()
}

fn cone() -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::new("test", "strong-link", "0.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}
