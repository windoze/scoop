use super::*;

#[test]
fn ordinary_reader_retains_generic_bodies_from_actual_published_libraries() {
    let target = resolved_target().expect("generic body publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-generic-body-production");
    for case in ["standalone", "combined", "support"] {
        let source = std::fs::read_to_string(directory.join(format!("{case}.scoop"))).unwrap();
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
        let interface = closure.current_compile().production().hir_interface();
        let bodies = interface.generic_callable_bodies().records();
        assert!(
            bodies.iter().any(|body| matches!(
                body.owner(),
                scoop_hir::DefaultCallableDeclarationV1::GenericFunction(_)
            )),
            "{case}"
        );
        if case == "standalone" {
            assert_eq!(bodies.len(), 3);
            assert_eq!(interface.callable_interfaces().support_records().len(), 2);
        }
        if case == "combined" {
            assert!(bodies.iter().any(|body| !body.capture_types().is_empty()));
        }
        if case == "support" {
            assert_eq!(interface.nominal_interfaces().support_records().len(), 1);
            assert_eq!(interface.public_bindings().records().len(), 1);
        }
    }
}
