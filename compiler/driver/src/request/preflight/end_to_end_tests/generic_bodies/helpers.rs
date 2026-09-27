use super::super::imported_classes::runtime;
use super::*;

#[test]
fn generic_helpers_keep_definition_bindings_and_captures_through_artifacts() {
    let target = resolved_target().expect("generic helper publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-helper-consumption");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-helper-provider", "0.1.0").unwrap();
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
        let name = format!("generic-helper-{case}");
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
        let peer = if case == "combined" {
            let name = "generic-helper-peer";
            let root = sysroot.path().join(name);
            write_manifest_cone(
                &root,
                "dev.example",
                name,
                "library",
                &source("combined-peer"),
            );
            write_dependency_manifest(&root, name, &[&provider_coordinate]);
            let output = build_manifest_request(
                sysroot.path(),
                &target,
                &root,
                &sysroot.path().join("output/peer.slib"),
                vec![provider.artifact().path().to_path_buf()],
                Vec::new(),
            )
            .build_and_publish()
            .unwrap();
            std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
            Some((
                ConeCoordinate::new("dev.example", name, "0.1.0").unwrap(),
                output,
            ))
        } else {
            None
        };
        let downstream_name = format!("generic-helper-downstream-{case}");
        let downstream_root = sysroot.path().join(&downstream_name);
        write_manifest_cone(
            &downstream_root,
            "dev.example",
            &downstream_name,
            "library",
            &source(if case == "combined" {
                "combined-downstream"
            } else {
                "downstream"
            }),
        );
        let mut direct_coordinates = vec![&coordinate];
        let mut direct_artifacts = vec![consumer.artifact().path().to_path_buf()];
        if let Some((coordinate, output)) = &peer {
            direct_coordinates.push(coordinate);
            direct_artifacts.push(output.artifact().path().to_path_buf());
        }
        write_dependency_manifest(&downstream_root, &downstream_name, &direct_coordinates);
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &downstream_root,
            &sysroot
                .path()
                .join(format!("output/{downstream_name}.slib")),
            direct_artifacts,
            vec![provider.artifact().path().to_path_buf()],
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} downstream: {error:?}"));
        let mut artifacts = vec![&core, &provider, consumer];
        if let Some((_, output)) = &peer {
            artifacts.push(output);
        }
        artifacts.push(&downstream);
        let closure = runtime::check(
            &target,
            &artifacts,
            &runtime,
            &runtime_fixtures,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
        if let Some((peer, _)) = peer {
            let providers = [
                provider_coordinate.identity().unwrap(),
                coordinate.identity().unwrap(),
                peer.identity().unwrap(),
            ];
            let shared = closure
                .odr_definitions()
                .members()
                .filter(|member| {
                    member.key().role() == scoop_identity::OdrMemberRole::CallableBody
                        && member.candidates().count() == 3
                        && member
                            .candidates()
                            .all(|candidate| providers.contains(&candidate.provider()))
                })
                .count();
            // Six ordinary generic bodies and three lexical implementations.
            assert!(
                shared >= 9,
                "all shared helper bodies must coalesce: {shared}"
            );
        }
    }

    for case in [
        "bad-private-generic",
        "bad-private-ordinary",
        "bad-local-lookup",
        "bad-no-gc",
        "bad-mutable-capture",
    ] {
        let root = sysroot.path().join(case);
        let source = source(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        write_dependency_manifest(&root, case, &[&provider_coordinate]);
        let destination = sysroot.path().join(format!("output/{case}.slib"));
        let error = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &destination,
            vec![provider.artifact().path().to_path_buf()],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap_err();
        let SingleConeProductionError::Production(error) = error else {
            panic!("{case} must be diagnosed during compilation: {error:?}")
        };
        let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
            diagnostics,
        )) = error.cause()
        else {
            panic!("{case} must be diagnosed before MIR: {error:?}")
        };
        let actual = diagnostics
            .iter()
            .map(|diagnostic| {
                assert_eq!(
                    diagnostic.file, 0,
                    "the error belongs to the consumer source"
                );
                let span = diagnostic.span.unwrap();
                format!(
                    "span={}..{}\nexpression={}\n{}\n",
                    span.start,
                    span.end,
                    &source[span.start as usize..span.end as usize],
                    diagnostic.message
                )
            })
            .collect::<String>();
        let snapshot = fixtures.join(format!("{case}.diagnostic.snap"));
        if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, &actual).unwrap();
        }
        assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap(), "{case}");
        assert!(!destination.exists());
    }
}
