use scoop_identity::ConeCoordinate;
use scoop_lir::ValidatedLirTargetSelection;

use super::*;
use crate::{
    MemberPurposeSet, MetadataSection, hir_identity_foundation_capability,
    lir_cross_cone_layout_abi_capability, mir_identity_foundation_capability,
};

fn compatibility() -> CompatibilityRecord {
    CompatibilityRecord::new(
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        crate::ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
    )
    .unwrap()
}

fn section(layer: FoundationLayer, payload: &[u8]) -> MetadataSection {
    let capability = match layer {
        FoundationLayer::Hir => hir_identity_foundation_capability(),
        FoundationLayer::Mir => mir_identity_foundation_capability(),
        FoundationLayer::Lir => lir_cross_cone_layout_abi_capability(),
    };
    MetadataSection::new(
        layer.location(),
        capability,
        MemberPurposeSet::COMPILE_AND_LINK,
        payload.to_vec(),
    )
    .unwrap()
}

fn dependency(name: &str, hir: u8, mir: u8, lir: u8) -> DependencyRecord {
    DependencyRecord::new(
        ConeCoordinate::new("example", name, "0.1.0").unwrap(),
        HirFingerprint::from_array([hir; 32]),
        MirFingerprint::from_array([mir; 32]),
        LirFingerprint::from_array([lir; 32]),
    )
    .unwrap()
}

fn semantic_fingerprints(
    compatibility: &CompatibilityRecord,
    dependencies: &[DependencyRecord],
    hir: &MetadataSection,
    mir: &MetadataSection,
    lir: &MetadataSection,
) -> Result<SemanticFingerprintRecord, SemanticFingerprintError> {
    SemanticFingerprintRecord::from_metadata_sections(
        compatibility,
        dependencies,
        std::slice::from_ref(hir),
        std::slice::from_ref(mir),
        std::slice::from_ref(lir),
    )
}

#[test]
fn foundation_fingerprints_have_fixed_vectors() {
    let fingerprints = semantic_fingerprints(
        &compatibility(),
        &[],
        &section(FoundationLayer::Hir, b"hir"),
        &section(FoundationLayer::Mir, b"mir"),
        &section(FoundationLayer::Lir, b"lir"),
    )
    .unwrap();

    assert_eq!(
        [
            fingerprints.hir().to_string(),
            fingerprints.mir().to_string(),
            fingerprints.lir().to_string()
        ],
        [
            "ff93330e0bbcf11a497ad603a6e21abf0e313e5d43ca9e6136b0965047279bee",
            "3653391c3c8c1a08a7446fc8a4f9bd9fdaf6e77a745ef57cf91090c3bf9b6efe",
            "3e66e691594c18aa4ba9c5cb2463f18e945888f1249be0276e046012b59e7091",
        ]
    );
}

#[test]
fn payload_and_dependency_fingerprints_affect_only_their_layer() {
    let original_dependency = dependency("left", 1, 2, 3);
    let changed_dependency = dependency("left", 9, 2, 3);
    let hir = section(FoundationLayer::Hir, b"hir");
    let mir = section(FoundationLayer::Mir, b"mir");
    let lir = section(FoundationLayer::Lir, b"lir");
    let original = semantic_fingerprints(
        &compatibility(),
        std::slice::from_ref(&original_dependency),
        &hir,
        &mir,
        &lir,
    )
    .unwrap();
    let dependency_changed = semantic_fingerprints(
        &compatibility(),
        std::slice::from_ref(&changed_dependency),
        &hir,
        &mir,
        &lir,
    )
    .unwrap();
    let payload_changed = semantic_fingerprints(
        &compatibility(),
        std::slice::from_ref(&original_dependency),
        &section(FoundationLayer::Hir, b"changed"),
        &mir,
        &lir,
    )
    .unwrap();

    assert_ne!(original.hir(), dependency_changed.hir());
    assert_eq!(original.mir(), dependency_changed.mir());
    assert_eq!(original.lir(), dependency_changed.lir());
    assert_ne!(original.hir(), payload_changed.hir());
    assert_eq!(original.mir(), payload_changed.mir());
    assert_eq!(original.lir(), payload_changed.lir());
}

#[test]
fn multiple_registry_contributions_are_canonical_and_layer_local() {
    let foundation = section(FoundationLayer::Hir, b"foundation");
    let production = MetadataSection::new(
        MetadataLocation::Hir,
        crate::hir_core_bootstrap_interface_capability(),
        MemberPurposeSet::COMPILE,
        b"production".to_vec(),
    )
    .unwrap();
    let changed_production = MetadataSection::new(
        MetadataLocation::Hir,
        crate::hir_core_bootstrap_interface_capability(),
        MemberPurposeSet::COMPILE,
        b"changed-production".to_vec(),
    )
    .unwrap();
    let mir = section(FoundationLayer::Mir, b"mir");
    let lir = section(FoundationLayer::Lir, b"lir");

    let forward = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility(),
        &[],
        &[foundation.clone(), production.clone()],
        std::slice::from_ref(&mir),
        std::slice::from_ref(&lir),
    )
    .unwrap();
    let reverse = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility(),
        &[],
        &[production, foundation.clone()],
        std::slice::from_ref(&mir),
        std::slice::from_ref(&lir),
    )
    .unwrap();
    let changed = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility(),
        &[],
        &[foundation.clone(), changed_production],
        std::slice::from_ref(&mir),
        std::slice::from_ref(&lir),
    )
    .unwrap();

    assert_eq!(forward, reverse);
    assert_ne!(forward.hir(), changed.hir());
    assert_eq!(forward.mir(), changed.mir());
    assert_eq!(forward.lir(), changed.lir());
    assert!(matches!(
        SemanticFingerprintRecord::from_metadata_sections(
            &compatibility(),
            &[],
            &[foundation.clone(), foundation],
            std::slice::from_ref(&mir),
            std::slice::from_ref(&lir),
        ),
        Err(SemanticFingerprintError::DuplicateContribution {
            layer: FoundationLayer::Hir,
            ..
        })
    ));
}

#[test]
fn optional_unknown_sections_are_envelope_only() {
    let hir = section(FoundationLayer::Hir, b"hir");
    let optional = MetadataSection::new(
        MetadataLocation::Hir,
        CapabilityId::new("org.scoop-lang.test", "optional", 1).unwrap(),
        MemberPurposeSet::NONE,
        b"ignored".to_vec(),
    )
    .unwrap();
    let mir = section(FoundationLayer::Mir, b"mir");
    let lir = section(FoundationLayer::Lir, b"lir");
    let baseline = semantic_fingerprints(&compatibility(), &[], &hir, &mir, &lir).unwrap();
    let with_optional = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility(),
        &[],
        &[hir, optional],
        std::slice::from_ref(&mir),
        std::slice::from_ref(&lir),
    )
    .unwrap();

    assert_eq!(with_optional, baseline);
}

#[test]
fn dependency_order_is_canonical_and_duplicates_are_rejected() {
    let left = dependency("left", 1, 2, 3);
    let right = dependency("right", 4, 5, 6);
    let hir = section(FoundationLayer::Hir, b"hir");
    let mir = section(FoundationLayer::Mir, b"mir");
    let lir = section(FoundationLayer::Lir, b"lir");
    let forward = semantic_fingerprints(
        &compatibility(),
        &[left.clone(), right.clone()],
        &hir,
        &mir,
        &lir,
    )
    .unwrap();
    let reverse =
        semantic_fingerprints(&compatibility(), &[right, left.clone()], &hir, &mir, &lir).unwrap();

    assert_eq!(forward, reverse);
    assert!(matches!(
        semantic_fingerprints(&compatibility(), &[left.clone(), left], &hir, &mir, &lir,),
        Err(SemanticFingerprintError::DuplicateDependency { .. })
    ));
}

#[test]
fn foundation_calculator_rejects_a_section_from_another_layer() {
    let hir = section(FoundationLayer::Hir, b"hir");
    let mir = section(FoundationLayer::Mir, b"mir");
    assert_eq!(
        semantic_fingerprints(&compatibility(), &[], &hir, &mir, &mir,),
        Err(SemanticFingerprintError::WrongSectionLocation {
            layer: FoundationLayer::Lir,
            index: 0,
            actual: MetadataLocation::Mir,
        })
    );
}

#[test]
fn private_lir_production_does_not_change_dependency_semantics() {
    let compatibility = compatibility();
    let hir = [section(FoundationLayer::Hir, b"template")];
    let mir = [section(FoundationLayer::Mir, b"interface")];
    let exported = section(FoundationLayer::Lir, b"public ABI");
    let calculate = |lir: &[MetadataSection]| {
        SemanticFingerprintRecord::from_metadata_sections(&compatibility, &[], &hir, &mir, lir)
            .unwrap()
    };
    let baseline = calculate(std::slice::from_ref(&exported));
    for capability in [
        crate::lir_identity_foundation_capability(),
        crate::lir_strong_production_capability(),
        crate::lir_cone_production_capability(),
    ] {
        for payload in [b"debug private roots".as_slice(), b"release private roots"] {
            let private = MetadataSection::new(
                MetadataLocation::Lir,
                capability.clone(),
                MemberPurposeSet::COMPILE_AND_LINK,
                payload.to_vec(),
            )
            .unwrap();
            assert_eq!(calculate(&[exported.clone(), private]), baseline);
        }
    }
    let changed_abi = calculate(&[section(FoundationLayer::Lir, b"changed public ABI")]);
    assert_eq!(changed_abi.hir(), baseline.hir());
    assert_eq!(changed_abi.mir(), baseline.mir());
    assert_ne!(changed_abi.lir(), baseline.lir());
}
