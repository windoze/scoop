use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn retired_artifact_profiles_require_rebuilding() {
    let selection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let current =
        CompatibilityRecord::new(selection, ArtifactCapabilityProfile::CROSS_CONE_GENERIC).unwrap();
    for name in ["single-cone-strong", "cross-cone-semantics-strong"] {
        let mut decoded: DecodedCompatibilityRecord =
            decode_canonical(&encode(&current).unwrap()).unwrap();
        let retired =
            scoop_identity::CapabilityId::new("org.scoop-lang.slib-profile", name, 5).unwrap();
        decoded.artifact_profile = decode_canonical(&encode(&retired).unwrap()).unwrap();
        assert!(matches!(
            decoded.validate(selection),
            Err(CompatibilityValidationError::ArtifactProfile(_))
        ));
    }
}

#[test]
fn unknown_or_mismatched_backend_identity_is_rejected_even_with_known_fingerprint() {
    let selection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let record =
        CompatibilityRecord::new(selection, ArtifactCapabilityProfile::CROSS_CONE_GENERIC).unwrap();
    for (name, version) in [
        ("future-backend", 1),
        ("llvm-22-1", 2),
        ("llvm-22-1-linux-x86-64", 1),
    ] {
        let mut decoded: DecodedCompatibilityRecord =
            decode_canonical(&encode(&record).unwrap()).unwrap();
        let capability =
            scoop_identity::CapabilityId::new("org.scoop-lang.backend-profile", name, version)
                .unwrap();
        decoded.backend_profile = decode_canonical(&encode(&capability).unwrap()).unwrap();
        assert!(matches!(
            decoded.validate(selection),
            Err(CompatibilityValidationError::ProfileMismatch {
                kind: CompatibilityProfileKind::Backend,
                ..
            })
        ));
    }
}

#[test]
fn compatibility_reader_selects_the_exact_registered_artifact_profile() {
    let selection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let profile = ArtifactCapabilityProfile::CROSS_CONE_GENERIC;

    let expected = CompatibilityRecord::new(selection, profile).unwrap();
    let decoded =
        decode_canonical::<DecodedCompatibilityRecord>(&encode(&expected).unwrap()).unwrap();
    let actual = decoded.validate(selection).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.artifact_profile(), &profile.id());
}

#[test]
fn compatibility_reader_rejects_each_old_outer_schema_and_mixed_generations() {
    let selection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let current =
        CompatibilityRecord::new(selection, ArtifactCapabilityProfile::CROSS_CONE_GENERIC).unwrap();
    for old_schema in 1..4 {
        for (kind, schemas) in [
            (CompatibilitySchemaKind::Hir, (old_schema, 4, 4)),
            (CompatibilitySchemaKind::Mir, (4, old_schema, 4)),
            (CompatibilitySchemaKind::Lir, (4, 4, old_schema)),
            (
                CompatibilitySchemaKind::Hir,
                (old_schema, old_schema, old_schema),
            ),
        ] {
            let mut decoded =
                decode_canonical::<DecodedCompatibilityRecord>(&encode(&current).unwrap()).unwrap();
            (decoded.hir_schema, decoded.mir_schema, decoded.lir_schema) = schemas;
            assert!(
                matches!(decoded.validate(selection), Err(CompatibilityValidationError::UnsupportedSchema { kind: actual_kind, actual }) if actual_kind == kind && actual == old_schema)
            );
        }
    }
}

#[test]
fn canonical_unknown_target_is_rejected_even_with_matching_fingerprints() {
    const UNKNOWN: scoop_lir::TargetProfileId =
        match scoop_lir::TargetProfileId::new("unknown-target", 1) {
            Ok(id) => id,
            Err(_) => panic!("canonical test identity"),
        };
    let target =
        scoop_lir::LirTargetProfile::new(&UNKNOWN, scoop_target_data::DARWIN_AARCH64.description());
    let selection =
        scoop_lir::ValidatedLirTargetSelection::new(target, scoop_target_data::LLVM_22_1);
    let record =
        CompatibilityRecord::new(selection, ArtifactCapabilityProfile::CROSS_CONE_GENERIC).unwrap();
    let decoded: DecodedCompatibilityRecord = decode_canonical(&encode(&record).unwrap()).unwrap();
    assert!(matches!(
        decoded.validate(selection),
        Err(CompatibilityValidationError::UnsupportedSelection)
    ));
}

#[test]
fn target_identity_must_match_the_selected_consumer_profile() {
    let selection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let record =
        CompatibilityRecord::new(selection, ArtifactCapabilityProfile::CROSS_CONE_GENERIC).unwrap();
    for (name, version) in [
        ("unknown-target", 1),
        ("darwin-aarch64", 2),
        ("linux-x86-64-gnu", 1),
    ] {
        let mut decoded: DecodedCompatibilityRecord =
            decode_canonical(&encode(&record).unwrap()).unwrap();
        let identity =
            scoop_identity::CapabilityId::new("org.scoop-lang.target-profile", name, version)
                .unwrap();
        decoded.target_profile = decode_canonical(&encode(&identity).unwrap()).unwrap();
        assert!(matches!(
            decoded.validate(selection),
            Err(CompatibilityValidationError::ProfileMismatch {
                kind: CompatibilityProfileKind::Target,
                ..
            })
        ));
    }
}
