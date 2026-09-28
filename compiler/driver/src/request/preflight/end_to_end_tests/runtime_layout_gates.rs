//! Runtime exception storage is materialized from the shared source declarations.

use super::*;

#[test]
fn runtime_exception_storage_is_materialized_before_mir_and_publication() {
    let target = resolved_target().expect("runtime layout fixtures require the host target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-runtime-layout-gates");
    for name in [
        "standalone",
        "combined",
        "arithmetic",
        "arithmetic-default",
        "inactive",
        "arithmetic-inactive",
    ] {
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        let root = sysroot.path().join(name);
        write_manifest_cone(&root, "dev.example", name, "library", &source);
        let destination = root.join("output.slib");
        let request = SingleConeBuildRequest::new(
            CurrentConeInput::Manifest {
                root: ManifestRootLocator::cone_directory(&root),
            },
            ExplicitDependencyInputs::new(vec![], vec![]),
            TrustedCoreInput::Artifact(HostArtifactLocator::new(core.artifact().path()).unwrap()),
            target.clone(),
            SlibOutputDestination::new(destination.clone()).unwrap(),
            DiagnosticOutputPolicy::Human,
            StageDumpPolicy::Stage(StageDumpKind::Mir),
        )
        .unwrap();
        let artifact = request
            .build_and_publish()
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let dump = artifact.emitted_dump().unwrap();
        assert_eq!(dump.kind(), StageDumpKind::Mir);
        snapshot(&fixtures.join(format!("{name}.mir.snap")), dump.text());
        assert!(destination.is_file());
    }
}

fn snapshot(path: &Path, text: &str) {
    if std::env::var_os("SCOOP_UPDATE_RUNTIME_LAYOUT_GATES").is_some() {
        std::fs::write(path, text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
