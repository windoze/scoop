use super::*;

#[test]
fn ordinary_reader_replays_default_call_domains_from_published_bytes() {
    let target = resolved_target().expect("default call domain publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-default-call-domains");
    for (case, expected_templates, expected_inherited) in
        [("standalone", 1, 0), ("combined", 11, 6)]
    {
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
        if std::env::var_os("SCOOP_UPDATE_CALL_DOMAINS_SNAPSHOTS").is_some() {
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
        let interface = closure.current_compile().production().hir_interface();
        let templates = interface.default_templates().records();
        assert_eq!(templates.len(), expected_templates, "{case}");
        assert_eq!(
            templates
                .iter()
                .filter(
                    |template| template.definition_root().declaration() != template.key().owner()
                )
                .count(),
            expected_inherited,
            "{case}"
        );
        if case == "combined" {
            let mut logical_only_slot = false;
            let mut applied_mapping = false;
            let mut direct_generic = false;
            for template in templates {
                let callable = interface
                    .callable_interfaces()
                    .get(template.key().owner())
                    .unwrap();
                logical_only_slot |= callable.access()
                    == scoop_hir::PublicLookupAccessV1::PublicSlot
                    && callable.slot_relations().values().is_empty();
                applied_mapping |= template
                    .type_parameters()
                    .arguments()
                    .iter()
                    .any(|ty| matches!(ty, scoop_identity::SignatureTypeKey::Tuple(_)));
                direct_generic |= matches!(
                    template.key().owner(),
                    scoop_identity::CallableTemplateOrigin::GenericFunction(_)
                ) && callable.access()
                    == scoop_hir::PublicLookupAccessV1::DirectOnly;
            }
            assert!(logical_only_slot && applied_mapping && direct_generic);
        }
    }
}
