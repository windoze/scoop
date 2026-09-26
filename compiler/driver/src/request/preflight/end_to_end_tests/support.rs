use super::*;

pub(super) fn resolved_target() -> Option<scoop_toolchain::ResolvedTargetProfile> {
    // The target resolver owns host Apple-toolchain qualification. A machine
    // blocked by the Xcode license gate cannot enter object production.
    let target = scoop_toolchain::ResolvedTargetProfile::resolve_host().ok()?;
    scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection()).ok()?;
    Some(target)
}

pub(super) fn bootstrap_core(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
) -> SingleConeProductionSuccess {
    copy_trusted_core_sources(sysroot);
    let slot =
        crate::trusted_core::resolve_trusted_core_slot_at(sysroot, target.lir_target_selection())
            .unwrap();
    let artifact_path = slot.artifact().to_path_buf();
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(slot.source_root()),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::BootstrapSelf,
        target.clone(),
        SlibOutputDestination::new(&artifact_path).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish()
    .unwrap()
}

pub(super) fn build_manifest(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    root: &Path,
    output: &Path,
) -> SingleConeProductionSuccess {
    build_ordinary(
        sysroot,
        target,
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(root),
        },
        output,
    )
}

pub(super) fn build_single_file(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    source: &Path,
    output: &Path,
) -> SingleConeProductionSuccess {
    build_ordinary(
        sysroot,
        target,
        CurrentConeInput::SingleFile {
            source: scoop_manifest::SingleFileLocator::from_path(source).unwrap(),
        },
        output,
    )
}

pub(super) fn build_ordinary(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    current: CurrentConeInput,
    output: &Path,
) -> SingleConeProductionSuccess {
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let core_slot =
        crate::trusted_core::resolve_trusted_core_slot_at(sysroot, target.lir_target_selection())
            .unwrap();
    SingleConeBuildRequest::new(
        current,
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core_slot.artifact()).unwrap()),
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish()
    .unwrap()
}

pub(super) fn build_manifest_request(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    root: &Path,
    output: &Path,
    direct: Vec<std::path::PathBuf>,
    support: Vec<std::path::PathBuf>,
) -> SingleConeBuildRequest {
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let core_slot =
        crate::trusted_core::resolve_trusted_core_slot_at(sysroot, target.lir_target_selection())
            .unwrap();
    SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(root),
        },
        ExplicitDependencyInputs::new(
            direct
                .into_iter()
                .map(HostArtifactLocator::new)
                .collect::<Result<_, _>>()
                .unwrap(),
            support
                .into_iter()
                .map(HostArtifactLocator::new)
                .collect::<Result<_, _>>()
                .unwrap(),
        ),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core_slot.artifact()).unwrap()),
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
}

pub(super) fn assert_graph_dependencies(
    artifact: &SingleConeProductionSuccess,
    target: &scoop_toolchain::ResolvedTargetProfile,
    expected: &[ConeIdentity],
) {
    let bytes = std::fs::read(artifact.artifact().path()).unwrap();
    let graph = DecodedSlibEnvelope::open(&bytes, target.lir_target_selection())
        .unwrap()
        .validate_graph()
        .unwrap();
    assert_eq!(
        graph
            .direct_dependencies()
            .iter()
            .map(scoop_slib::DependencyRecord::identity)
            .collect::<Vec<_>>(),
        expected
    );
}

pub(super) fn assert_artifacts_equal(
    first: &SingleConeProductionSuccess,
    second: &SingleConeProductionSuccess,
) {
    assert_eq!(
        std::fs::read(first.artifact().path()).unwrap(),
        std::fs::read(second.artifact().path()).unwrap()
    );
    assert_eq!(
        first.artifact().summary().artifact_fingerprint(),
        second.artifact().summary().artifact_fingerprint()
    );
}

pub(super) fn write_manifest_cone(root: &Path, group: &str, name: &str, kind: &str, source: &str) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        format!(
            "schema = 1\n[cone]\ngroup = \"{group}\"\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"{kind}\"\n"
        ),
    )
    .unwrap();
    std::fs::write(root.join("src/main.scoop"), source).unwrap();
}

pub(super) fn write_dependency_manifest(
    root: &Path,
    name: &str,
    dependencies: &[&ConeCoordinate],
) -> String {
    std::fs::create_dir_all(root).unwrap();
    let mut manifest = format!(
        "schema = 1\n[cone]\ngroup = \"dev.example\"\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"library\"\n"
    );
    if !dependencies.is_empty() {
        manifest.push_str("[dependencies]\n");
        for coordinate in dependencies {
            manifest.push_str(&format!(
                "\"{}:{}\" = \"{}\"\n",
                coordinate.group(),
                coordinate.name(),
                coordinate.version()
            ));
        }
    }
    std::fs::write(root.join("Cone.toml"), &manifest).unwrap();
    manifest
}

pub(super) fn copy_trusted_core_sources(sysroot: &Path) {
    let source = crate::workspace_root().join("sysroot/lib/scoop.core");
    let destination = sysroot.join("lib/scoop.core");
    std::fs::create_dir_all(destination.join("src")).unwrap();
    std::fs::copy(source.join("Cone.toml"), destination.join("Cone.toml")).unwrap();
    for entry in std::fs::read_dir(source.join("src")).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        std::fs::copy(
            entry.path(),
            destination.join("src").join(entry.file_name()),
        )
        .unwrap();
    }
    std::fs::write(
        destination.join("src/stage3_test.scoop"),
        "public fun stage3CoreAnswer(): Long = 42\n\
         public fun stage3CoreFailure() { throw ArithmeticException() }\n",
    )
    .unwrap();
}
