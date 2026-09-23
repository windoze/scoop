use super::*;

#[test]
fn ordinary_reader_preserves_complete_shared_source_defaults() {
    let target = resolved_target().expect("shared source publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-shared-source-closure");
    for (case, expected_templates) in [("standalone", 2), ("combined", 14)] {
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
        let library = request.build_and_publish(DecodeLimits::default()).unwrap();
        let dump = library.emitted_dump().unwrap();
        let snapshot = directory.join(format!("{case}.hir.snap"));
        if std::env::var_os("SCOOP_UPDATE_SOURCE_CLOSURE_SNAPSHOTS").is_some() {
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
            DecodeLimits::default(),
            target.c_bridge_toolchain().profile(),
            &mut session,
        )
        .unwrap();
        let interface = closure.current_compile().production().hir_interface();
        let templates = interface.default_templates().records();
        assert_eq!(templates.len(), expected_templates, "{case}");
        for template in templates {
            assert!(
                interface
                    .callable_interfaces()
                    .get(template.key().owner())
                    .is_none()
            );
            assert!(
                interface
                    .callable_interfaces()
                    .declaration(template.key().owner())
                    .is_some()
            );
            assert!(
                interface
                    .source_interfaces()
                    .get(template.key().owner())
                    .is_some()
            );
        }
        let callables = interface.callable_interfaces();
        assert!(
            callables
                .support_records()
                .iter()
                .any(|record| record.owner() == scoop_hir::PublicDeclarationOwnerV1::TopLevel)
        );
        assert!(
            interface
                .property_interfaces()
                .support_records()
                .iter()
                .any(|record| record.owner() == scoop_hir::PublicDeclarationOwnerV1::TopLevel)
        );
        if case == "combined" {
            assert!(
                callables
                    .support_records()
                    .iter()
                    .any(|record| record.owner() == scoop_hir::PublicDeclarationOwnerV1::Extension)
            );
            assert!(
                templates
                    .iter()
                    .any(|template| template.definition_root().declaration()
                        != template.key().owner())
            );
            assert!(interface.nominal_interfaces().support_records().len() >= 3);
        }
        // Unreachable source defaults are neither roots nor promoted support.
        assert_eq!(
            callables
                .support_records()
                .iter()
                .filter(
                    |record| record.owner() == scoop_hir::PublicDeclarationOwnerV1::TopLevel
                        && matches!(
                            record.declaration(),
                            scoop_identity::CallableTemplateOrigin::Function(_)
                                | scoop_identity::CallableTemplateOrigin::GenericFunction(_)
                        )
                )
                .count(),
            1
        );
    }
}
