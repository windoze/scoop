use super::*;

#[test]
fn ordinary_reader_replays_complete_default_body_references_from_published_bytes() {
    let target = resolved_target().expect("default reference publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-default-reference-reader");
    for (case, expected_templates) in [("standalone", 1), ("combined", 4)] {
        let source = std::fs::read_to_string(directory.join(format!("{case}.scoop"))).unwrap();
        let root = sysroot.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        let mut request = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{case}.slib")),
            vec![],
            vec![],
        );
        request.emit = StageDumpPolicy::Stage(StageDumpKind::Hir);
        let library = request.build_and_publish().unwrap();
        let dump = library.emitted_dump().unwrap();
        let snapshot = directory.join(format!("{case}.hir.snap"));
        if std::env::var_os("SCOOP_UPDATE_REFERENCE_CLOSURE_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, dump.text()).unwrap();
        }
        assert_eq!(dump.text(), std::fs::read_to_string(snapshot).unwrap());
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
        let templates = closure
            .current_compile()
            .production()
            .hir_interface()
            .default_templates()
            .records();
        assert_eq!(templates.len(), expected_templates, "{case}");
        let mut counts = [0; 6];
        for template in templates {
            let references = template.references();
            for (count, length) in counts.iter_mut().zip([
                references.callables().len(),
                references.constructors().len(),
                references.types().len(),
                references.globals().len(),
                references.singleton_values().len(),
                references.fields().len(),
            ]) {
                *count += length;
            }
        }
        if case == "combined" {
            assert!(counts.into_iter().all(|count| count > 0), "{counts:?}");
        }
    }
}
