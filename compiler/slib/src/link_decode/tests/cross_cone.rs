use scoop_hir::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1,
};
use scoop_lir::CrossConeLirBridgeSectionV1;
use scoop_mir::CrossConeMirBridgeSectionV1;

use super::*;

#[test]
fn cross_cone_artifact_writer_is_byte_reproducible() {
    assert_eq!(
        complete_cross_cone_artifact(),
        complete_cross_cone_artifact()
    );
}

fn complete_cross_cone_artifact() -> Vec<u8> {
    let mut hir_foundation = scoop_hir::CanonicalHirFoundation::empty();
    hir_foundation
        .set_types(vec![
            scoop_identity::CoreBuiltinNominal::Unit.identity_record(),
            scoop_identity::CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    let hir_proof = scoop_hir::OdrFreeHirFoundation::try_new(hir_foundation).unwrap();
    let hir_production = decode_canonical::<scoop_hir::DecodedCoreBootstrapInterfaceSectionV1>(
        &empty_hir_library_section(),
    )
    .unwrap()
    .validate_against_strong_foundation(cone().identity(), &hir_proof)
    .unwrap();
    let hir_cross_cone = empty_hir_interface();

    let mut mir_foundation = scoop_mir::CanonicalMirFoundation::empty();
    mir_foundation
        .set_exact_types(vec![
            CborIdentityRecord::from_key(scoop_identity::ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            ))
            .unwrap(),
        ])
        .unwrap();
    let mir_proof = scoop_mir::OdrFreeMirFoundation::try_new(mir_foundation).unwrap();
    let mir_production =
        CrossConeMirBridgeSectionV1::try_new(cone().identity(), &mir_proof, Vec::new(), Vec::new())
            .unwrap();
    let core_mir_production = scoop_mir::CoreBootstrapBridgeSectionV1::try_new(
        cone().identity(),
        scoop_mir::EntryMirBridgeBranchV1::Library,
        scoop_mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&mir_proof),
    )
    .unwrap();

    let (lir_foundation, _) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let lir_proof = OdrFreeLirFoundation::try_new(cone().identity(), lir_foundation).unwrap();
    let lir_cross_cone =
        CrossConeLirBridgeSectionV1::try_new(&lir_proof, Vec::new(), Vec::new()).unwrap();
    let (strong_production, final_objects, defined_symbols, undefined_partitions) =
        finalized_cross_cone_link_object_fixture(&lir_cross_cone);

    let dependencies = vec![
        DependencyRecord::new(
            ConeCoordinate::reserved_core(),
            HirFingerprint::from_array([1; 32]),
            crate::MirFingerprint::from_array([2; 32]),
            crate::LirFingerprint::from_array([3; 32]),
        )
        .unwrap(),
    ];
    let plan = link_object_plan();
    let member_plan = &plan.scoop_lir_members()[0];
    let link_member = SlibMember::new(
        cone().identity(),
        member_plan.stable_key().clone(),
        member_plan.role().clone(),
        final_objects.objects()[0].bytes().to_vec(),
    )
    .unwrap();
    let link_objects =
        crate::verify_code_link_object_members_v1(final_objects, &[link_member.record().clone()])
            .unwrap();
    let code_projection = crate::verify_single_cone_production_code_projection_v1(
        &cone(),
        &dependencies,
        hir_proof.as_canonical().counts().sources,
        strong_production.clone(),
        link_objects,
    )
    .unwrap();
    let native = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        selection().target(),
        &lir_proof,
    )
    .unwrap();
    let cross_cone_code = crate::compute_cross_cone_code_fingerprint_v1(
        code_projection,
        native,
        defined_symbols,
        undefined_partitions,
    )
    .unwrap();
    let artifact = crate::AssembledCrossConeStrongArtifactV1::write(
        crate::CrossConeStrongArtifactInputV1::new(
            ProducerRecord::new("test").unwrap(),
            cone(),
            dependencies,
            &hir_proof,
            &hir_production,
            hir_cross_cone,
            &mir_proof,
            &core_mir_production,
            &mir_production,
            &lir_proof,
            &lir_cross_cone,
            cross_cone_code,
            vec![link_member],
        ),
    )
    .unwrap();
    artifact.as_bytes().to_vec()
}

fn empty_hir_interface() -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}
