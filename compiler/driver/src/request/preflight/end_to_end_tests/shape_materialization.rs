use super::*;

#[test]
fn ordinary_public_nominals_materialize_and_publish_the_shared_shape_plan() {
    let target = resolved_target().expect("shape production requires the host target");
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
        let request = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{fixture}.slib")),
            Vec::new(),
            Vec::new(),
        );
        let library = request.build_and_publish().unwrap();
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
