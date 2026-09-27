use super::*;

#[test]
fn actual_duplicate_generic_members_reject_different_bodies_with_equal_abis() {
    let target = resolved_target().expect("ODR conflict validation requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-odr-siblings");
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-odr-conflict-provider", "0.1.0").unwrap();
    let mut closures = Vec::new();
    let mut identities = Vec::new();
    for side in ["left", "right"] {
        let provider_root = sysroot.path().join(format!("provider-{side}"));
        let source =
            std::fs::read_to_string(fixtures.join(format!("conflict-provider-{side}.scoop")))
                .unwrap();
        write_manifest_cone(
            &provider_root,
            "dev.example",
            provider_coordinate.name(),
            "library",
            &source,
        );
        let provider = build_manifest(
            sysroot.path(),
            &target,
            &provider_root,
            &sysroot.path().join(format!("output/provider-{side}.slib")),
        );
        std::fs::rename(
            provider_root.join("src"),
            provider_root.join("unused-source"),
        )
        .unwrap();
        let name = format!("generic-odr-conflict-{side}");
        let root = sysroot.path().join(&name);
        let source = std::fs::read_to_string(fixtures.join("conflict-consumer.scoop")).unwrap();
        write_manifest_cone(&root, "dev.example", &name, "library", &source);
        write_dependency_manifest(&root, &name, &[&provider_coordinate]);
        let consumer = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            vec![provider.artifact().path().to_path_buf()],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{side}: {error:?}"));
        let identity = consumer
            .artifact()
            .summary()
            .coordinate()
            .identity()
            .unwrap();
        let mut direct = consumer
            .artifact()
            .summary()
            .direct_dependencies()
            .iter()
            .map(scoop_slib::DependencyRecord::identity)
            .collect::<Vec<_>>();
        direct.sort_unstable();
        let provider_bytes = std::fs::read(provider.artifact().path()).unwrap();
        let consumer_bytes = std::fs::read(consumer.artifact().path()).unwrap();
        let closure = scoop_slib::read_cross_cone_layout_artifact_closure(
            scoop_slib::CrossConeArtifactClosureInput::completed(
                identity,
                target.lir_target_selection(),
                direct,
                vec![&core_bytes, &provider_bytes],
                &consumer_bytes,
            ),
            target.c_bridge_toolchain().profile(),
        )
        .unwrap();
        closures.push(closure);
        identities.push(identity);
    }
    let mut different = false;
    for first in closures[0].odr_definitions().members() {
        let second = closures[1]
            .odr_definitions()
            .get(first.key().group(), first.member())
            .expect("the same generic application retains its group and member keys");
        assert_eq!(first.group_key(), second.group_key());
        assert_eq!(first.key(), second.key());
        assert_eq!(first.abi(), second.abi());
        different |= first.definition() != second.definition();
    }
    assert!(
        different,
        "the changed body must change actual definition content"
    );
    let first = closures[0].artifact(identities[0]).unwrap();
    let second = closures[1].artifact(identities[1]).unwrap();
    let error = scoop_slib::merge_cross_cone_odr_definitions([first, second]).unwrap_err();
    let scoop_slib::OdrDefinitionMergeError::Conflict(conflict) = error else {
        panic!("expected a duplicate member conflict: {error:?}");
    };
    assert_eq!(conflict.first, identities[0]);
    assert_eq!(conflict.second, identities[1]);
    assert!(
        closures[0]
            .odr_definitions()
            .get(conflict.group, conflict.member)
            .is_some()
    );
    assert!(
        matches!(
            conflict.difference,
            scoop_slib::OdrDefinitionDifference::Lir
                | scoop_slib::OdrDefinitionDifference::Object
                | scoop_slib::OdrDefinitionDifference::Stackmap
        ),
        "{conflict:?}"
    );
    assert!(matches!(
        scoop_slib::merge_cross_cone_odr_definitions([first, first]),
        Err(scoop_slib::OdrDefinitionMergeError::DuplicateArtifact(provider)) if provider == identities[0]
    ));
}
