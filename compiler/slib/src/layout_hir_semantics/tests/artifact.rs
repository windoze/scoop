use scoop_hir::{
    CanonicalExactTypeFactsV1, CanonicalExportDefinitionSourcesV1,
    CanonicalNominalInheritanceInterfacesV1, CanonicalNominalRepresentationSupportV1,
    CanonicalProtectedCallableSourceInterfacesV1, CanonicalProtectedDeclarationInterfacesV1,
    CanonicalProtectedDefaultTemplatesV1, CanonicalSelectedExternalTypeUsesV1,
    CrossConeTypeSemanticsSectionV1, SelectedExternalTypeUseV1,
};
use scoop_identity::ValidatedIdentityGraph;
use scoop_wire::{BudgetMeter, DecodeLimits, encode};

use crate::{
    ArtifactCapabilityProfile, DependencyRecord, HirProductionValidatedCrossConeLayoutSections,
    MemberPurposeSet, MetadataLocation, hir_cross_cone_interface_capability,
    hir_cross_cone_type_semantics_capability, hir_identity_foundation_capability,
    lir_cross_cone_layout_abi_capability, lir_cross_cone_layout_link_closure_capability,
    lir_cross_cone_link_closure_capability, lir_cross_cone_param_free_bridge_capability,
    lir_identity_foundation_capability, lir_strong_production_capability,
    lir_strong_production_v2_capability, mir_cross_cone_param_free_bridge_capability,
    mir_cross_cone_type_bridge_capability,
    strong_compile_decode::tests::{
        build_artifact_for_profile_with_dependencies, cone_named, open_graph, required_sections,
        section,
    },
};

use super::type_authority::NominalFixture;

pub(super) fn target() -> scoop_lir::ValidatedLirTargetSelection {
    scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}

pub(super) fn artifact_bytes(name: &str) -> Vec<u8> {
    artifact_bytes_with_dependencies(name, Vec::new())
}

pub(super) fn artifact_bytes_with_dependencies(
    name: &str,
    dependencies: Vec<DependencyRecord>,
) -> Vec<u8> {
    artifact_bytes_with_hir_semantics(name, dependencies, None, None)
}

pub(super) fn nominal_artifact_bytes(name: &str) -> (Vec<u8>, NominalFixture) {
    let nominal = NominalFixture::new(cone_named(name).identity());
    let semantics = nominal.section();
    let bytes =
        artifact_bytes_with_hir_semantics(name, Vec::new(), Some(&nominal), Some(&semantics));
    (bytes, nominal)
}

pub(super) fn selecting_artifact_bytes(
    name: &str,
    selected: SelectedExternalTypeUseV1,
    dependencies: Vec<DependencyRecord>,
) -> Vec<u8> {
    let semantics = empty_type_semantics(vec![selected]);
    artifact_bytes_with_hir_semantics(name, dependencies, None, Some(&semantics))
}

fn artifact_bytes_with_hir_semantics(
    name: &str,
    dependencies: Vec<DependencyRecord>,
    nominal: Option<&NominalFixture>,
    semantics: Option<&CrossConeTypeSemanticsSectionV1>,
) -> Vec<u8> {
    artifact_bytes_with_mir_type_bridge(
        name,
        dependencies,
        nominal,
        semantics,
        empty_array_fields(7),
    )
}

pub(super) fn artifact_bytes_with_mir_type_bridge(
    name: &str,
    dependencies: Vec<DependencyRecord>,
    nominal: Option<&NominalFixture>,
    semantics: Option<&CrossConeTypeSemanticsSectionV1>,
    mir_type_bridge: Vec<u8>,
) -> Vec<u8> {
    let cone = cone_named(name);
    let (mut hir, mut mir, mut lir) = required_sections();
    if let Some(nominal) = nominal {
        let foundation = hir
            .iter_mut()
            .find(|section| section.capability() == &hir_identity_foundation_capability())
            .expect("the shared fixture must contain the HIR identity foundation");
        *foundation = section(
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&nominal.foundation()).unwrap(),
        );
    }
    let type_semantics = semantics.map_or_else(
        || empty_array_fields(8),
        |semantics| {
            let mut meter = BudgetMeter::new(DecodeLimits::default());
            encode(&semantics.index_for_wire(&mut meter).unwrap()).unwrap()
        },
    );
    hir.extend([
        section(
            MetadataLocation::Hir,
            hir_cross_cone_interface_capability(),
            MemberPurposeSet::COMPILE,
            crate::cross_cone_compile_decode::tests::empty_cross_cone_hir_interface(),
        ),
        section(
            MetadataLocation::Hir,
            hir_cross_cone_type_semantics_capability(),
            MemberPurposeSet::COMPILE,
            type_semantics,
        ),
    ]);
    mir.extend([
        section(
            MetadataLocation::Mir,
            mir_cross_cone_param_free_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_array_fields(2),
        ),
        section(
            MetadataLocation::Mir,
            mir_cross_cone_type_bridge_capability(),
            MemberPurposeSet::COMPILE,
            mir_type_bridge,
        ),
    ]);

    let (foundation, production) =
        crate::link_decode::strong_production_fixture_for_test(cone.coordinate().clone());
    let foundation_section = lir
        .iter_mut()
        .find(|section| section.capability() == &lir_identity_foundation_capability())
        .expect("the shared fixture must contain the LIR identity foundation");
    *foundation_section = section(
        MetadataLocation::Lir,
        lir_identity_foundation_capability(),
        MemberPurposeSet::COMPILE,
        encode(&foundation).unwrap(),
    );
    lir.retain(|section| section.capability() != &lir_strong_production_capability());
    lir.extend([
        section(
            MetadataLocation::Lir,
            lir_strong_production_v2_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&production).unwrap(),
        ),
        section(
            MetadataLocation::Lir,
            lir_cross_cone_param_free_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_array_fields(2),
        ),
        section(
            MetadataLocation::Lir,
            lir_cross_cone_link_closure_capability(),
            MemberPurposeSet::LINK,
            vec![0x80],
        ),
        section(
            MetadataLocation::Lir,
            lir_cross_cone_layout_abi_capability(),
            MemberPurposeSet::COMPILE,
            empty_layout_abi(),
        ),
        section(
            MetadataLocation::Lir,
            lir_cross_cone_layout_link_closure_capability(),
            MemberPurposeSet::LINK,
            vec![0xff],
        ),
    ]);
    build_artifact_for_profile_with_dependencies(
        cone,
        ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
        dependencies,
        hir,
        mir,
        lir,
        false,
    )
}

pub(super) fn dependency_record(bytes: &[u8]) -> DependencyRecord {
    open_graph(bytes)
        .decode_cross_cone_layout_compile_sections()
        .unwrap()
        .dependency_record()
}

fn empty_type_semantics(
    selected: Vec<SelectedExternalTypeUseV1>,
) -> CrossConeTypeSemanticsSectionV1 {
    CrossConeTypeSemanticsSectionV1::new(
        CanonicalExactTypeFactsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalRepresentationSupportV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInheritanceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalProtectedDeclarationInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalProtectedCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalProtectedDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalSelectedExternalTypeUsesV1::try_new(selected).unwrap(),
    )
}

pub(super) fn checked_artifact(bytes: &[u8]) -> HirProductionValidatedCrossConeLayoutSections<'_> {
    checked_artifact_with_authorities(bytes, std::iter::empty())
}

pub(super) fn checked_artifact_with_authorities<'input, 'authority>(
    bytes: &'input [u8],
    authorities: impl IntoIterator<Item = &'authority ValidatedIdentityGraph>,
) -> HirProductionValidatedCrossConeLayoutSections<'input> {
    open_graph(bytes)
        .decode_cross_cone_layout_compile_sections()
        .unwrap()
        .validate_foundation_identities(authorities)
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .resolve_hir_sections()
        .unwrap()
        .validate_hir_production()
        .unwrap()
}

pub(super) fn identity_graph<'artifact>(
    artifact: &'artifact mut HirProductionValidatedCrossConeLayoutSections<'_>,
) -> &'artifact ValidatedIdentityGraph {
    artifact.hir_semantic_parts().0
}

fn empty_array_fields(count: u8) -> Vec<u8> {
    let mut bytes = vec![0xa0 | count];
    for field in 1..=count {
        bytes.extend([field, 0x80]);
    }
    bytes
}

fn empty_layout_abi() -> Vec<u8> {
    let mut bytes = empty_array_fields(5);
    bytes[0] = 0xa6;
    bytes.extend([0x06, 0xa2, 0x01, 0x80, 0x02, 0x80]);
    bytes
}
