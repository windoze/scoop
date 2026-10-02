use super::*;

#[test]
fn zero_sized_fields_and_captures_publish_without_payload_memory_operations() {
    let target = resolved_target().expect("ZST heap storage requires the supported target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-heap-zst");
    for case in ["standalone", "combined"] {
        let source = std::fs::read_to_string(directory.join(format!("{case}.scoop"))).unwrap();
        let root = sysroot.path().join(case);
        let name = format!("heap-zst-{case}");
        write_manifest_cone(&root, "dev.example", &name, "library", &source);
        for (kind, stage) in [
            (StageDumpKind::Hir, "hir"),
            (StageDumpKind::Mir, "mir"),
            (StageDumpKind::Lir, "lir"),
        ] {
            let mut request = build_manifest_request(
                sysroot.path(),
                &target,
                &root,
                &sysroot.path().join(format!("output/{case}-{stage}.slib")),
                vec![],
                vec![],
            );
            request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
            let library = request.build_and_publish().unwrap();
            let dump = library.emitted_dumps().first().unwrap();
            let snapshot = directory.join(format!("{case}.{stage}.snap"));
            if std::env::var_os("SCOOP_UPDATE_HEAP_ZST").is_some() {
                std::fs::write(&snapshot, dump.text()).unwrap();
            }
            assert_eq!(dump.text(), std::fs::read_to_string(snapshot).unwrap());
            let bytes = std::fs::read(library.artifact().path()).unwrap();
            let identity = ConeCoordinate::new("dev.example", &name, "0.1.0")
                .unwrap()
                .identity()
                .unwrap();
            scoop_slib::validate_completed_cross_cone_artifact_closure(
                identity,
                target.lir_target_selection(),
                vec![ConeIdentity::CORE],
                vec![&core_bytes],
                &bytes,
                target.c_bridge_toolchain().profile(),
                &mut scoop_identity::SemanticIdentitySession::new(),
            )
            .unwrap();
        }
    }
}
