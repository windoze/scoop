use super::*;

#[test]
fn real_process_builds_and_reuses_a_source_dependency() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let dependency = workspace.join("dependency");
    let root = workspace.join("root");
    copy_real_core(&sysroot);
    write_manifest_source(
        &dependency,
        "dependency",
        "library",
        "package dependency.api\n\npublic fun value(): Int = 1\n",
        "",
    );
    write_manifest_source(
        &root,
        "root",
        "library",
        "package consumer\n\nimport dependency.api.value\n\npublic fun run(): Int = value()\n",
        "[dependencies]\n\"test:dependency\" = { version = \"1.0.0\", path = \"../dependency\" }\n",
    );
    let dependency_identity = ConeCoordinate::new("test", "dependency", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let root_identity = ConeCoordinate::new("test", "root", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let build_request = || real_manifest_request(&root, workspace, &sysroot, &compiler);

    let mut first_runner = RecordingProductionRunner::default();
    let first = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut first_runner)
        .unwrap();
    assert_eq!(first_runner.current.len(), 3);
    assert_manifest_current_identity(&first_runner.current[0], ConeIdentity::CORE);
    assert_manifest_current_identity(&first_runner.current[1], dependency_identity);
    assert_manifest_current_identity(&first_runner.current[2], root_identity);
    assert_eq!(
        first.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::Compiled
    );
    let root_node = first.completed(root_identity).unwrap();
    let artifact = root_node.closure().artifact(root_identity).unwrap();
    assert_eq!(artifact.summary().cone().identity(), root_identity);
    assert!(std::ptr::eq(artifact, root_node.artifact()));
    assert!(matches!(
        first.into_outcome(),
        BuildGraphOutcome::Library { .. }
    ));

    let mut second_runner = RecordingProductionRunner::default();
    let second = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut second_runner)
        .unwrap();
    assert!(second_runner.current.is_empty());
    assert_eq!(
        second.completed(dependency_identity).unwrap().origin(),
        CompletedNodeOrigin::CacheHit
    );
    assert_eq!(
        second.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::CacheHit
    );

    let second = second.into_outcome();
    assert_eq!(second.artifacts().len(), 3);
    for node in second.artifacts() {
        assert!(!node.materialized_child_path().as_path().exists());
        assert_eq!(
            std::fs::read(node.artifact_locator()).unwrap(),
            node.artifact().snapshot().as_bytes()
        );
    }

    let dump_root = workspace.join("dumps");
    let mut observed = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap();
    observed
        .observe_dumps(crate::BuildDumpRequest {
            stages: scoop_protocol::StageDumpSet::all(),
            directory: dump_root.clone(),
            scope: crate::DumpScope::Root,
        })
        .unwrap();
    let mut observed_runner = RecordingProductionRunner::default();
    let observed = observed
        .execute_with_runner(&mut observed_runner)
        .unwrap()
        .into_outcome();
    assert_eq!(observed_runner.current.len(), 1);
    assert_eq!(
        observed.observations().child_invocations(),
        &[root_identity]
    );
    assert_eq!(
        observed.root().artifact().snapshot().as_bytes(),
        second.root().artifact().snapshot().as_bytes()
    );
    assert_eq!(std::fs::read_dir(&dump_root).unwrap().count(), 1);
    for stage in scoop_protocol::StageDumpKindV1::ALL {
        assert!(
            dump_root
                .join(root_identity.to_string())
                .join(stage.file_name())
                .is_file()
        );
    }
    let hir =
        std::fs::read_to_string(dump_root.join(root_identity.to_string()).join("hir.txt")).unwrap();
    assert!(hir.contains("== Export ==\n"));
    assert!(hir.contains("== LocalConcrete ==\n"));

    std::fs::write(dependency.join("src/warning.scoop"), "enum Signal { Ready, Failed }\nfun describe(value: Signal): Int = when (value) {\nReady -> 1\nRaedy -> 2\n}\n").unwrap();
    std::fs::write(
        root.join("src/main.scoop"),
        "fun broken(): Int = missingName\n",
    )
    .unwrap();
    for _ in 0..2 {
        let failure = build_request()
            .load_root()
            .unwrap()
            .discover()
            .unwrap()
            .resolve()
            .unwrap()
            .prepare()
            .unwrap()
            .execute()
            .unwrap_err();
        assert_eq!(failure.warnings().len(), 1);
        assert!(
            matches!(failure.warnings()[0].origin(), scoop_protocol::DiagnosticOriginV1::SemanticSourceSpan { cone, logical_path, .. }
            if cone.as_array() == dependency_identity.as_array() && logical_path.as_str() == "src/warning.scoop")
        );
        assert!(!failure.child_diagnostics().unwrap().is_empty());
    }
}
