//! Actual implicit construction must pass shared source materialization first.

use super::*;

const MESSAGE: &str = "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED: runtime cast failure constructor requires a materialized dependency layout; its owner has source-only representation";

#[test]
fn runtime_cast_source_only_layout_is_diagnosed_before_mir_and_publication() {
    let target = resolved_target().expect("runtime layout fixtures require the host target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-runtime-layout-gates");
    for name in ["standalone", "combined"] {
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        let root = sysroot.path().join(name);
        write_manifest_cone(&root, "dev.example", name, "library", &source);
        let destination = root.join("output.slib");
        let error =
            build_manifest_request(sysroot.path(), &target, &root, &destination, vec![], vec![])
                .build_and_publish()
                .unwrap_err();
        let SingleConeProductionError::Production(error) = error else {
            panic!("{name}: rejection must occur in the current HIR stage")
        };
        let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
            diagnostics,
        )) = error.cause()
        else {
            panic!("{name}: runtime layout must be checked before MIR: {error:?}")
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, MESSAGE);
        let span = diagnostics[0].span.unwrap();
        assert!(span.start < span.end);
        let expression = &source[span.start as usize..span.end as usize];
        let dump = format!(
            "span={}..{}\nexpression={expression}\n{MESSAGE}\n",
            span.start, span.end
        );
        snapshot(&fixtures.join(format!("{name}.diagnostic.snap")), &dump);
        assert!(!destination.exists());
    }

    let name = "inactive";
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
        target,
        SlibOutputDestination::new(destination.clone()).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::Stage(StageDumpKind::Mir),
    )
    .unwrap();
    let artifact = request.build_and_publish().unwrap();
    let dump = artifact.emitted_dump().unwrap();
    assert_eq!(dump.kind(), StageDumpKind::Mir);
    snapshot(&fixtures.join("inactive.mir.snap"), dump.text());
    assert!(destination.is_file());
}

fn snapshot(path: &Path, text: &str) {
    if std::env::var_os("SCOOP_UPDATE_RUNTIME_LAYOUT_GATES").is_some() {
        std::fs::write(path, text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
