use super::*;

#[test]
fn formal_publication_keeps_unrequested_nominals_as_complete_source_interfaces() {
    let target = resolved_target().expect("source-only publication requires the supported target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    for case in ["standalone", "combined", "shape-demand"] {
        let source = std::fs::read_to_string(crate::workspace_root().join(format!(
            "tests/fixtures/m23-source-only-nominals/{case}.scoop"
        )))
        .unwrap();
        let root = sysroot.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        let library = build_manifest(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{case}.slib")),
        );
        let bytes = std::fs::read(library.artifact().path()).unwrap();
        let identity = ConeCoordinate::new("dev.example", case, "0.1.0")
            .unwrap()
            .identity()
            .unwrap();
        let mut session = scoop_identity::SemanticIdentitySession::new();
        let closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
            identity,
            target.lir_target_selection(),
            vec![ConeIdentity::CORE],
            vec![&core_bytes],
            &bytes,
            target.c_bridge_toolchain().profile(),
            &mut session,
        )
        .unwrap();
        let production = closure.current_compile().production();
        assert!(
            production
                .hir_interface()
                .nominal_interfaces()
                .records()
                .len()
                > production
                    .lir_strong()
                    .shape_support_plan()
                    .closures()
                    .len()
        );
    }
}

#[test]
fn source_only_objects_preserve_required_initialization_through_all_emitted_stages() {
    let target = resolved_target().expect("source-only publication requires the supported target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-source-only-nominals");
    let source = std::fs::read_to_string(directory.join("initialization-demand.scoop")).unwrap();
    let root = sysroot.path().join("initialization-demand");
    write_manifest_cone(
        &root,
        "dev.example",
        "initialization-demand",
        "library",
        &source,
    );
    for (kind, stage) in [
        (StageDumpKind::Hir, "hir"),
        (StageDumpKind::Mir, "mir"),
        (StageDumpKind::Lir, "lir"),
    ] {
        let mut request = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot
                .path()
                .join(format!("output/initialization-{stage}.slib")),
            Vec::new(),
            Vec::new(),
        );
        request.emit = StageDumpPolicy::Stage(kind);
        let library = request.build_and_publish().unwrap();
        let dump = library.emitted_dump().unwrap();
        let snapshot = directory.join(format!("initialization-demand.{stage}.snap"));
        if std::env::var_os("SCOOP_UPDATE_AUTOMATIC_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, dump.text()).unwrap();
        }
        assert_eq!(dump.text(), std::fs::read_to_string(snapshot).unwrap());
        let bytes = std::fs::read(library.artifact().path()).unwrap();
        let identity = ConeCoordinate::new("dev.example", "initialization-demand", "0.1.0")
            .unwrap()
            .identity()
            .unwrap();
        let mut session = scoop_identity::SemanticIdentitySession::new();
        let closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
            identity,
            target.lir_target_selection(),
            vec![ConeIdentity::CORE],
            vec![&core_bytes],
            &bytes,
            target.c_bridge_toolchain().profile(),
            &mut session,
        )
        .unwrap();
        assert_eq!(
            closure
                .current_compile()
                .production()
                .lir_strong()
                .shape_support_plan()
                .closures()
                .len(),
            2
        );
    }
}
