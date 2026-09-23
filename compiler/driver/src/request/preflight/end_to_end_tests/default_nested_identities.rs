use super::*;
use scoop_hir::DefaultSourceNestedCallableDescriptorV1 as Descriptor;
use scoop_wire::{BudgetMeter, WirePath};

#[test]
fn ordinary_reader_binds_nested_default_identities_from_published_bytes() {
    let target = resolved_target().expect("nested identity publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-default-nested-identities");
    for (case, expected_templates) in [("standalone", 1), ("combined", 7)] {
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
        if std::env::var_os("SCOOP_UPDATE_NESTED_IDENTITY_SNAPSHOTS").is_some() {
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
        let templates = closure
            .current_compile()
            .production()
            .hir_interface()
            .default_templates()
            .records();
        assert_eq!(templates.len(), expected_templates, "{case}");
        let mut kinds = [false; 4];
        let mut expanded_arguments = false;
        for template in templates {
            let nested = template
                .index_nested_callables(
                    &mut BudgetMeter::new(DecodeLimits::default()),
                    &WirePath::root(),
                )
                .unwrap();
            assert_eq!(nested.template(), template.key());
            for occurrence in nested.occurrences() {
                let descriptor = occurrence.descriptor();
                kinds[match descriptor {
                    Descriptor::LocalFunction(_) => 0,
                    Descriptor::Lambda(_) => 1,
                    Descriptor::AnonymousFunction(_) => 2,
                    Descriptor::CallableReference(_) => 3,
                }] = true;
                if let scoop_hir::DefaultNestedCallableBodyArgumentsV1::Explicit(arguments) =
                    descriptor.body_arguments()
                {
                    assert_eq!(
                        arguments.len() as u32,
                        descriptor.owner_type_parameter_count()
                    );
                    expanded_arguments = true;
                }
            }
        }
        assert!(kinds[1]);
        if case == "combined" {
            assert!(kinds.into_iter().all(|seen| seen), "{kinds:?}");
            assert!(expanded_arguments);
        }
    }
}
