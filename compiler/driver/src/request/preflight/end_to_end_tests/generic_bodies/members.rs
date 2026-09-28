use super::super::imported_classes::runtime;
use super::*;

#[test]
fn generic_member_templates_republish_and_execute_from_artifacts() {
    let target = resolved_target().expect("generic member publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-member-consumption");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-member-provider", "0.1.0").unwrap();
    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        provider_coordinate.name(),
        "library",
        &source("provider"),
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("output/provider.slib"),
    );
    std::fs::rename(
        provider_root.join("src"),
        provider_root.join("unused-source"),
    )
    .unwrap();
    let runtime = runtime::build(&target, &sysroot.path().join("runtime"));
    let runtime_fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-classes");
    for case in [
        "standalone",
        "method-arguments",
        "signature-support",
        "value-method",
        "plain-owner",
        "inherited",
        "captured",
        "virtual",
        "abi",
        "overloads",
        "interface-class",
        "interface-abstract",
        "interface-struct",
        "interface-enum",
        "interface-local",
        "interface-abi",
        "interface-properties",
        "interface-value-property",
    ] {
        eprintln!("generic member case: {case}");
        let name = format!("generic-member-{case}");
        let coordinate = ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap();
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        write_dependency_manifest(&root, &name, &[&provider_coordinate]);
        let mut outputs = Vec::new();
        for (kind, stage) in [
            (StageDumpKind::Hir, "hir"),
            (StageDumpKind::Mir, "mir"),
            (StageDumpKind::Lir, "lir"),
        ] {
            let mut request = build_manifest_request(
                sysroot.path(),
                &target,
                &root,
                &sysroot.path().join(format!("output/{name}.slib")),
                vec![provider.artifact().path().to_path_buf()],
                Vec::new(),
            );
            request.emit = StageDumpPolicy::Stage(kind);
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            let snapshot = fixtures.join(format!("{case}.{stage}.snap"));
            let actual = output.emitted_dump().unwrap().text();
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            outputs.push(output);
        }
        let consumer = outputs.last().unwrap();
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        let downstream_name = format!("generic-member-downstream-{case}");
        let downstream_root = sysroot.path().join(&downstream_name);
        write_manifest_cone(
            &downstream_root,
            "dev.example",
            &downstream_name,
            "library",
            &source("downstream"),
        );
        write_dependency_manifest(&downstream_root, &downstream_name, &[&coordinate]);
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &downstream_root,
            &sysroot
                .path()
                .join(format!("output/{downstream_name}.slib")),
            vec![consumer.artifact().path().to_path_buf()],
            vec![provider.artifact().path().to_path_buf()],
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} downstream: {error:?}"));
        let closure = runtime::check(
            &target,
            &[&core, &provider, consumer, &downstream],
            &runtime,
            &runtime_fixtures,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
        let (provider_sections, _) = closure
            .artifact(provider_coordinate.identity().unwrap())
            .unwrap();
        assert!(
            provider_sections
                .lir_strong_production()
                .canonical_shape_definitions()
                .definitions()
                .is_empty(),
            "the provider has not instantiated these generic types"
        );
        let (sections, _) = closure.artifact(coordinate.identity().unwrap()).unwrap();
        let mut methods = 0;
        for binding in sections
            .mir_type_bridge()
            .exports()
            .callables()
            .entries()
            .iter()
            .filter(|binding| {
                matches!(
                    binding.implementation(),
                    scoop_identity::CallableDefinitionOwner::Odr(_)
                )
            })
        {
            let abi = sections
                .lir_exports()
                .callables()
                .get(binding.implementation())
                .expect("a method has its actual lowered ABI");
            assert_eq!(
                abi.canonical_signature().signature(),
                binding.lowered_signature().exact()
            );
            assert_eq!(
                abi.definition().symbol().linkage(),
                scoop_identity::LinkageClass::OdrWeak
            );
            assert_eq!(
                abi.physical_definition().provider(),
                coordinate.identity().unwrap()
            );
            if matches!(
                binding.lowering_role(),
                scoop_mir::MirCallableLoweringRoleV1::Ordinary
            ) {
                methods += 1;
            }
        }
        assert!(methods > 0, "{case} has actual method definitions");
    }
}
