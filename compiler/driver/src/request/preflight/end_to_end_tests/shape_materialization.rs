use super::*;

#[test]
fn ordinary_public_nominals_materialize_and_publish_the_shared_shape_plan() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    for (fixture, expected_roots, expected_boxes) in [("value", 1, 1), ("combined", 4, 2)] {
        let source = std::fs::read_to_string(crate::workspace_root().join(format!(
            "tests/fixtures/m23-shape-materialization/{fixture}.scoop"
        )))
        .unwrap();
        let root = sysroot.path().join(fixture);
        write_manifest_cone(&root, "dev.example", fixture, "library", &source);
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
                    .join(format!("output/{fixture}-{stage}.slib")),
                Vec::new(),
                Vec::new(),
            );
            request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
            let library = request.build_and_publish().unwrap();
            let dump = library.emitted_dumps().first().unwrap();
            assert_eq!(dump.kind(), kind);
            snapshot(fixture, stage, dump.text());
            let identity = ConeCoordinate::new("dev.example", fixture, "0.1.0")
                .unwrap()
                .identity()
                .unwrap();
            let bytes = std::fs::read(library.artifact().path()).unwrap();
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
            let plan = production.lir_strong().shape_support_plan();
            assert_eq!(plan.closures().len(), expected_roots);
            assert_eq!(
                plan.closures()
                    .iter()
                    .filter(|root| { root.roles().boxed_value().available().is_some() })
                    .count(),
                expected_boxes
            );
            for root in plan.closures() {
                assert_eq!(root.root(), identity);
                let roles = root.roles();
                assert!(roles.value_layout().available().is_some());
                assert!(roles.ref_scan().available().is_some());
                assert!(roles.type_descriptor().available().is_some());
                assert!(roles.type_registration().available().is_some());
                assert!(roles.coroutine_step().available().is_some());
                assert!(roles.coroutine_slot().available().is_some());
            }
        }
    }
}

fn snapshot(fixture: &str, stage: &str, text: &str) {
    let name = format!("{fixture}.{stage}.snap");
    if let Some(directory) = std::env::var_os("SCOOP_SHAPE_SNAPSHOT_DIR") {
        let directory = Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(directory.join(name), text).unwrap();
        return;
    }
    let path = crate::workspace_root()
        .join("tests/fixtures/m23-shape-materialization")
        .join(name);
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
