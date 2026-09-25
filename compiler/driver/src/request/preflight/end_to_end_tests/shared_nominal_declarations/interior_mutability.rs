use super::*;

#[test]
fn published_shared_structs_preserve_interior_mutability_in_both_artifact_views() {
    let target = resolved_target().expect("publication requires a host target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-shared-interior-mutability");
    for (case, structs, marked) in [("standalone", 2, 1), ("combined", 5, 4)] {
        let source = std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap();
        let root = sysroot.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        let artifact = build_manifest(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{case}.slib")),
        );
        let bytes = std::fs::read(artifact.artifact().path()).unwrap();
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
        let counts = closure.current_compile().lir().counts();
        assert_eq!(counts.c_abi_layouts, usize::from(case == "combined"));
        assert_eq!(counts.c_abi_signatures, 0);
        assert_eq!(counts.native_contracts, 0);
        let table = closure
            .current_compile()
            .production()
            .hir_interface()
            .nominal_interfaces();
        let policies = table
            .all_records()
            .filter_map(|record| {
                if let scoop_hir::NominalSourceShapeV1::Struct(shape) = record.source_shape() {
                    Some(shape.interior_mutable())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(policies.len(), structs);
        assert_eq!(policies.iter().filter(|marked| **marked).count(), marked);
    }
}
