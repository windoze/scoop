use std::{fs, path::Path};

use scoop_identity::ConeCoordinate;
use scoop_slib::*;

use super::support::*;

// Only the container/manifest is consumed by PM. Compilation owns IR validation.
pub(super) fn artifact(
    path: &Path,
    coordinate: ConeCoordinate,
    dependencies: Vec<DependencyRecord>,
    seed: u8,
) -> DependencyRecord {
    let target = scoop_toolchain::select_target("aarch64-apple-darwin").unwrap();
    let cone = ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap();
    let members = [
        (MemberStableKey::HirMetadata, SlibMemberRole::HirMetadata),
        (MemberStableKey::MirMetadata, SlibMemberRole::MirMetadata),
        (MemberStableKey::LirMetadata, SlibMemberRole::LirMetadata),
    ]
    .into_iter()
    .map(|(key, role)| {
        SlibMember::new(cone.identity(), key, role, b"opaque metadata".to_vec()).unwrap()
    })
    .collect::<Vec<_>>();
    let compatibility =
        CompatibilityRecord::new(target, ArtifactCapabilityProfile::CROSS_CONE_GENERIC).unwrap();
    let semantic = SemanticFingerprintRecord::from_digests(
        HirFingerprint::from_array([seed; 32]),
        MirFingerprint::from_array([seed; 32]),
        LirFingerprint::from_array([seed; 32]),
        FingerprintAvailability::Available(CodeFingerprint::from_array([seed; 32])),
        FingerprintAvailability::Available(RuntimeImageFingerprint::from_array([seed; 32])),
    );
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("pm-format-test").unwrap(),
        compatibility,
        cone,
        dependencies,
        &members,
        semantic,
        vec![],
    )
    .unwrap();
    let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, archive.as_bytes()).unwrap();
    read_artifact_manifest_summary(archive.as_bytes(), target)
        .unwrap()
        .dependency_record()
}

#[test]
fn artifact_coordinates_targets_and_recorded_semantics_remain_exact() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let core_path = root.join("sysroot/lib/aarch64-apple-darwin/scoop.core.slib");
    let core = artifact(&core_path, ConeCoordinate::reserved_core(), vec![], 1);
    artifact(
        &root.join("codec.slib"),
        ConeCoordinate::new("org.example", "codec", "1.0.0").unwrap(),
        vec![core],
        3,
    );
    let app = root.join("app");
    write(
        &app.join("Cone.toml"),
        &format!(
            "{}\n[dependencies]\n'org.example:codec'={{version='=1.0.0', artifact='../codec.slib'}}\n",
            cone("app", "0.1.0")
        ),
    );
    let args = [
        "resolve",
        "--target",
        "aarch64-apple-darwin",
        "--sysroot",
        "../sysroot",
    ];
    success(pm(root, &app, &args));
    assert_eq!(lock(&app).resolutions[0].nodes.len(), 3);
    success(pm(
        root,
        &app,
        &[
            "fetch",
            "--frozen",
            "--target",
            "aarch64-apple-darwin",
            "--sysroot",
            "../sysroot",
        ],
    ));
    let before = fs::read(app.join("Cone.lock")).unwrap();
    failure(
        pm(
            root,
            &app,
            &[
                "resolve",
                "--target",
                "x86_64-unknown-linux-gnu",
                "--sysroot",
                "../sysroot",
            ],
        ),
        "target",
    );
    artifact(&core_path, ConeCoordinate::reserved_core(), vec![], 2);
    failure(pm(root, &app, &args), "different semantic fingerprint");
    assert_eq!(fs::read(app.join("Cone.lock")).unwrap(), before);
}

#[test]
fn equal_coordinates_with_different_artifact_contents_are_ambiguous() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let core = artifact(
        &root.join("sysroot/lib/aarch64-apple-darwin/scoop.core.slib"),
        ConeCoordinate::reserved_core(),
        vec![],
        1,
    );
    for (directory, seed) in [("one", 2), ("two", 3)] {
        artifact(
            &root.join(directory).join("scoop/scoop.std/0.1.0/cone.slib"),
            ConeCoordinate::new("scoop", "scoop.std", "0.1.0").unwrap(),
            vec![core.clone()],
            seed,
        );
    }
    let app = root.join("app");
    write(
        &app.join("Cone.toml"),
        &format!(
            "{}\n[dependencies]\n'scoop:scoop.std'='0.1.0'\n",
            cone("app", "0.1.0")
        ),
    );
    failure(
        pm(
            root,
            &app,
            &[
                "resolve",
                "--target",
                "aarch64-apple-darwin",
                "--sysroot",
                "../sysroot",
                "--cone-path",
                "../one",
                "--cone-path",
                "../two",
            ],
        ),
        "ambiguous artifacts",
    );
    assert!(!app.join("Cone.lock").exists());
}
