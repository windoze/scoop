use scoop_identity::ConeCoordinate;
use scoop_lir::{
    CanonicalNativeExternalRequirementSurfaceV1, ConeLirFoundation, ConeProductionSectionV2,
    EntryProductionSourceV1, LirTargetProfile, StrongRegistrationProductionSurfaceV2,
    ValidatedLirTargetSelection,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::link_object::layout_link_closure::tests::fixture::empty_section;
use crate::link_object::native_requirements::tests::dependency_closure;
use crate::link_object::strong_relocation_closure::tests::verified_member_without_relocations;
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::link_object::undefined_requirements::tests::empty_bridge_plan;
use crate::{
    ConeKind, ConeRecord, ConeSourceForm, CrossConeLayoutProductionManifestV1,
    DecodedSingleConeProductionManifestV1, DependencyRecord, HirFingerprint, LirFingerprint,
    MirFingerprint, SingleConeProductionManifestValidationError,
    finalize_layout_partitioned_undefined_symbol_requirements_v1,
    seal_builtin_object_external_requirements_v1, verify_current_cone_undefined_requirements_v1,
    verify_dependency_strong_requirements_v1, verify_external_shape_requirements_v1,
    verify_runtime_and_eh_requirements_v1,
    verify_source_external_requirements_after_external_shape_v1,
};

#[test]
fn layout_code_fingerprint_binds_v2_imports_and_the_required_member_directory() {
    let cone = cone();
    let producer = cone.identity();
    let objects =
        crate::link_decode::tests::layout_link_support::verified_layout_code_link_object_members();
    let (canonical, v1) = crate::link_decode::tests::strong_production_fixture(
        cone.coordinate().clone(),
        &[scoop_identity::ConeIdentity::CORE],
    );
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();
    let target = LirTargetProfile::DARWIN_AARCH64;
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
        scoop_lir::CanonicalShapeLirDefinitionsV1::new(Vec::new(), &foundation).unwrap(),
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
    let callable = legacy_closure(strong.clone());
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
    let runtime = verify_runtime_and_eh_requirements_v1(
        source,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let external = seal_builtin_object_external_requirements_v1(
        crate::link_object::without_generated_bridge_semantics_for_test(runtime),
    )
    .unwrap();
    let partitions =
        finalize_layout_partitioned_undefined_symbol_requirements_v1(current, external, &shape)
            .unwrap();

    let wrong_strong = crate::verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&fixture_for_producer(producer, "wrongLayoutCode")),
    ])
    .unwrap();
    let wrong_callable = legacy_closure(wrong_strong);
    let wrong_shape = verify_external_shape_requirements_v1(
        &wrong_callable,
        layout.selected().consumer(),
        layout.selected().physical_imports(),
    )
    .unwrap();
    assert!(matches!(
        compute_cross_cone_layout_code_fingerprint_v1(
            production.clone(),
            native.clone(),
            defined.clone(),
            partitions.clone(),
            &wrong_shape,
        ),
        Err(LayoutCodeFingerprintError::ExternalShapeClosureMismatch)
    ));

    let proof = compute_cross_cone_layout_code_fingerprint_v1(
        production, native, defined, partitions, &shape,
    )
    .unwrap();
    assert_eq!(
        proof
            .code()
            .link_extension_contributions()
            .contributions()
            .len(),
        2
    );
    assert_eq!(
        encode(proof.layout_link_closure().semantic_imports()).unwrap(),
        encode(layout.selected().physical_imports()).unwrap()
    );
    let manifest = CrossConeLayoutProductionManifestV1::from_verified_code(proof.code().clone());
    let bytes = encode(&manifest).unwrap();
    let decoded = decode_canonical::<DecodedSingleConeProductionManifestV1>(&bytes).unwrap();
    assert_eq!(decoded.validate_layout(proof.code()).unwrap(), manifest);

    let mut changed = bytes;
    let offset = changed
        .windows(32)
        .position(|bytes| bytes == proof.code().fingerprint().as_array())
        .unwrap();
    changed[offset] ^= 1;
    let decoded = decode_canonical::<DecodedSingleConeProductionManifestV1>(&changed).unwrap();
    assert!(matches!(
        decoded.validate_layout(proof.code()),
        Err(SingleConeProductionManifestValidationError::ProjectionMismatch)
    ));
}

fn legacy_closure(
    strong: crate::VerifiedCurrentConeStrongRelocationClosureV1,
) -> crate::VerifiedCrossConeStrongRequirementClosureV1 {
    let producer = strong.producer();
    let object = fixture_for_producer(producer, "layoutCodeCoreOwner");
    let seed = dependency_closure(verified_member_without_relocations(&object));
    verify_dependency_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
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
