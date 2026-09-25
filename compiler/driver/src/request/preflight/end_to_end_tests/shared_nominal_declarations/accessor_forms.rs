use super::*;

#[test]
fn formal_publication_preserves_accessor_source_forms_in_both_artifact_views() {
    let target = resolved_target().expect("publication requires a host target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-core-layout-exports");
    for (case, expected) in [("standalone", [3, 0, 2, 0]), ("combined", [9, 1, 10, 2])] {
        let source =
            std::fs::read_to_string(fixtures.join(format!("shared-accessors-{case}.scoop")))
                .unwrap();
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
            DecodeLimits::default(),
            target.c_bridge_toolchain().profile(),
            &mut session,
        )
        .unwrap();
        let properties = closure
            .current_compile()
            .production()
            .hir_interface()
            .property_interfaces();
        let mut counts = [0; 4];
        for property in properties.all_declarations() {
            for source in std::iter::once(property.accessors().getter_source())
                .chain(property.accessors().setter_source())
            {
                use scoop_hir::PropertyAccessorImplementationV1 as Form;
                counts[match source.implementation() {
                    Form::Storage => 0,
                    Form::Constant => 1,
                    Form::Body => 2,
                    Form::AbstractSlot => 3,
                }] += 1;
            }
        }
        assert_eq!(counts, expected, "{case}");
    }
}
