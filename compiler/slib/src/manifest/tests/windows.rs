use super::*;
use scoop_identity::WindowsSubsystem;

#[test]
fn executable_subsystem_survives_manifest_records_without_changing_identity() {
    let original = ConeRecord::new(
        ConeCoordinate::reserved_single_file(),
        ConeKind::Executable,
        ConeSourceForm::SingleFile,
    )
    .unwrap();
    for subsystem in [
        None,
        Some(WindowsSubsystem::Console),
        Some(WindowsSubsystem::Gui),
    ] {
        let record = match subsystem {
            Some(value) => original.clone().with_windows_subsystem(value).unwrap(),
            None => original.clone(),
        };
        let bytes = encode(&record).unwrap();
        let decoded: DecodedConeRecord = scoop_wire::decode_canonical(&bytes).unwrap();
        let decoded = decoded.validate().unwrap();
        assert_eq!(decoded.windows_subsystem(), subsystem);
        assert_eq!(decoded.identity(), original.identity());
        assert_eq!(record, decoded);
    }
    assert!(
        cone()
            .with_windows_subsystem(WindowsSubsystem::Gui)
            .is_err()
    );
}

#[test]
fn subsystem_changes_artifact_fingerprint_and_rejects_unknown_wire_values() {
    let make = |subsystem| {
        let cone = ConeRecord::new(
            ConeCoordinate::reserved_single_file(),
            ConeKind::Executable,
            ConeSourceForm::SingleFile,
        )
        .unwrap()
        .with_windows_subsystem(subsystem)
        .unwrap();
        let members = [
            (MemberStableKey::HirMetadata, SlibMemberRole::HirMetadata),
            (MemberStableKey::MirMetadata, SlibMemberRole::MirMetadata),
            (MemberStableKey::LirMetadata, SlibMemberRole::LirMetadata),
        ]
        .map(|(key, role)| SlibMember::new(cone.identity(), key, role, Vec::new()).unwrap());
        BootstrapManifest::new(
            ProducerRecord::new("dev").unwrap(),
            CompatibilityRecord::new(
                scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1,
                ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
            )
            .unwrap(),
            cone,
            Vec::new(),
            &members,
            fingerprints(1),
            Vec::new(),
        )
        .unwrap()
    };
    let console = make(WindowsSubsystem::Console);
    let gui = make(WindowsSubsystem::Gui);
    assert_ne!(console.artifact_fingerprint(), gui.artifact_fingerprint());
    let mut bytes = encode(gui.cone()).unwrap();
    *bytes.last_mut().unwrap() = 3;
    let decoded: DecodedConeRecord = scoop_wire::decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.validate(),
        Err(ConeRecordValidationError::UnknownWindowsSubsystem { actual: 3 })
    ));
}
