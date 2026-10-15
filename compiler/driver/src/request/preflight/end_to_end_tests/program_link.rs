use super::*;
use scoop_slib::ArtifactFile;

#[test]
fn program_link_reads_machine_data_from_actual_executable_artifacts() {
    let target = resolved_target().expect("program Link fixtures require the target toolchain");
    let workspace = tempfile::tempdir().unwrap();
    let core = bootstrap_core(workspace.path(), &target);
    let core_file =
        ArtifactFile::open(core.artifact().path(), target.lir_target_selection()).unwrap();
    for case in ["read-basic", "read-combined", "read-println"] {
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
        let file = ArtifactFile::open(executable.artifact().path(), target.lir_target_selection())
            .unwrap();
        let closure = scoop_slib::read_program_link_closure(
            &file,
            &[&core_file, &core_file],
            target.lir_target_selection(),
        )
        .unwrap_or_else(|error| panic!("{case}: {error}"));
        assert_eq!(closure.artifacts().len(), 2);
        let (core, core_symbols) = closure.artifacts().next().unwrap();
        assert_eq!(core_symbols.link_support().runtime_data_aliases().len(), 1);
        let alias_bytes = scoop_wire::encode(core_symbols.link_support()).unwrap();
        let decoded = scoop_wire::decode_canonical::<scoop_slib::DecodedLirLinkSupportSectionV1>(
            &alias_bytes,
        )
        .unwrap();
        assert_eq!(scoop_wire::encode(&decoded).unwrap(), alias_bytes);
        assert!(core.production().entry_symbol.is_none());
        let (root, symbols) = closure.artifacts().last().unwrap();
        assert_eq!(root.identity(), closure.root());
        assert!(root.production().entry_symbol.is_some());
        assert_eq!(
            scoop_slib::FingerprintAvailability::Available(symbols.code_fingerprint()),
            root.manifest().semantic_fingerprints().code()
        );
        let missing =
            scoop_slib::read_program_link_closure(&file, &[], target.lir_target_selection())
                .err()
                .unwrap();
        assert!(missing.to_string().contains("MissingDirectArtifact"));
    }
    let library_root =
        scoop_slib::read_program_link_closure(&core_file, &[], target.lir_target_selection())
            .err()
            .unwrap();
    assert!(library_root.to_string().contains("executable Cone"));
}
