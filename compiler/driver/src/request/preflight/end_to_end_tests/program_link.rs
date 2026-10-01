use super::*;

#[test]
fn program_link_reads_machine_data_from_actual_executable_artifacts() {
    let target = resolved_target().expect("program Link fixtures require the target toolchain");
    let workspace = tempfile::tempdir().unwrap();
    let core = bootstrap_core(workspace.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    for case in ["read-basic", "read-combined"] {
        let source = std::fs::read_to_string(
            crate::workspace_root().join(format!("tests/fixtures/m23-program-link/{case}.scoop")),
        )
        .unwrap();
        let root = workspace.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "executable", &source);
        let executable = build_manifest(
            workspace.path(),
            &target,
            &root,
            &workspace.path().join(format!("output/{case}.slib")),
        );
        let bytes = std::fs::read(executable.artifact().path()).unwrap();
        let closure = scoop_slib::read_program_link_closure(
            &bytes,
            &[&core_bytes, &core_bytes],
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile(),
        )
        .unwrap_or_else(|error| panic!("{case}: {error}"));
        assert_eq!(closure.artifacts().len(), 2);
        let (root, symbols) = closure.artifacts().last().unwrap();
        assert_eq!(root.identity(), closure.root());
        assert!(matches!(
            root.production().entry_plan(),
            scoop_lir::EntryProductionPlanV1::Executable(_)
        ));
        assert_eq!(
            scoop_slib::FingerprintAvailability::Available(symbols.code_fingerprint()),
            root.manifest().semantic_fingerprints().code()
        );
        let missing = scoop_slib::read_program_link_closure(
            &bytes,
            &[],
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile(),
        )
        .err()
        .unwrap();
        assert!(missing.to_string().contains("MissingDirectArtifact"));
    }
    let library_root = scoop_slib::read_program_link_closure(
        &core_bytes,
        &[],
        target.lir_target_selection(),
        target.c_bridge_toolchain().profile(),
    )
    .err()
    .unwrap();
    assert!(library_root.to_string().contains("executable Cone"));
}
