use super::super::imported_classes::runtime;
use super::*;

#[test]
fn generic_nominal_consumers_create_payload_instances_from_artifacts() {
    let target = resolved_target().expect("generic nominal publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-nominal-consumption");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-nominal-provider", "0.1.0").unwrap();
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
    for case in ["standalone", "combined"] {
        let name = format!("generic-nominal-{case}");
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
            request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            let snapshot = fixtures.join(format!("{case}.{stage}.snap"));
            let actual = output.emitted_dumps().first().unwrap().text();
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            outputs.push(output);
        }
        let consumer = outputs.last().unwrap();
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        let downstream_name = format!("generic-nominal-downstream-{case}");
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
            "the provider has never instantiated these generic payloads"
        );
        let (sections, _) = closure.artifact(coordinate.identity().unwrap()).unwrap();
        let is_provider_nominal = |group| {
            let graph = sections.identity_graph();
            let key = graph
                .canonical_key::<_, scoop_identity::SpecializationKey>(group)
                .unwrap();
            let scoop_identity::SpecializationKey::Nominal { origin, .. } = key.as_ref() else {
                return false;
            };
            graph
                .canonical_key::<_, scoop_identity::SourceDeclarationKey>(*origin)
                .unwrap()
                .origin()
                == provider_coordinate.identity().unwrap()
        };
        let expected_types = if case == "standalone" { 1 } else { 4 };
        let registrations = sections
            .lir_strong_production()
            .type_registrations()
            .registrations()
            .iter()
            .filter(|registration| {
                matches!(registration.definition_owner(), scoop_lir::RegistrationDefinitionOwner::Odr { group, .. }
                    if is_provider_nominal(group))
            })
            .count();
        assert_eq!(registrations, expected_types);
        for shape in sections
            .lir_strong_production()
            .canonical_shape_definitions()
            .definitions()
        {
            let definition = closure
                .odr_definitions()
                .get(shape.group(), shape.member())
                .unwrap();
            assert_eq!(definition.abi().as_array(), shape.abi().as_array());
            assert!(
                definition
                    .candidates()
                    .any(|candidate| candidate.provider() == coordinate.identity().unwrap())
            );
        }
    }
}
