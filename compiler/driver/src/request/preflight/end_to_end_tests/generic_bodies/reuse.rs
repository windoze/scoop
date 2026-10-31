use super::*;
use scoop_identity::OdrMemberRole;
use scoop_lir::DebugInfoLevel;
use scoop_slib::{ArtifactFile, LinkDefinitionOwnerV1};
use std::collections::BTreeSet;

#[test]
fn dependency_odr_reuse_preserves_physical_bundles_and_debug_info() {
    let target = resolved_target().expect("ODR reuse requires a target");
    let workspace = tempfile::tempdir().unwrap();
    let core = bootstrap_core(workspace.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/language/generic-body-consumption");
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "reuse-provider", "0.1.0").unwrap();
    let first_coordinate = ConeCoordinate::new("dev.example", "reuse-first", "0.1.0").unwrap();
    let provider_root = workspace.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        provider_coordinate.name(),
        "library",
        &std::fs::read_to_string(fixtures.join("machine-provider.scoop")).unwrap(),
    );
    let provider = build_manifest(
        workspace.path(),
        &target,
        &provider_root,
        &workspace.path().join("provider.slib"),
    );
    let first_source = std::fs::read_to_string(fixtures.join("machine-combined.scoop")).unwrap();
    let first_root = workspace.path().join("first");
    write_manifest_cone(
        &first_root,
        "dev.example",
        first_coordinate.name(),
        "library",
        &first_source,
    );
    write_dependency_manifest(
        &first_root,
        first_coordinate.name(),
        &[&provider_coordinate],
    );
    let first = build_manifest_request(
        workspace.path(),
        &target,
        &first_root,
        &workspace.path().join("first.slib"),
        vec![provider.artifact().path().to_owned()],
        Vec::new(),
    )
    .build_and_publish()
    .unwrap();
    let dependencies = [
        core.artifact().path(),
        provider.artifact().path(),
        first.artifact().path(),
    ];
    let dependency_bytes = dependencies.map(|path| std::fs::read(path).unwrap());
    let dependency_files =
        dependencies.map(|path| ArtifactFile::open(path, target.lir_target_selection()).unwrap());

    for (name, debug_info) in [
        ("reuse-none", DebugInfoLevel::None),
        ("reuse-full", DebugInfoLevel::Full),
    ] {
        let root = workspace.path().join(name);
        let mut source = first_source.replacen(
            "package dev.example\n",
            "package dev.example.consumer\nimport dev.example.primitive as dependencyPrimitive\n",
            1,
        );
        source.push_str("\nfun main() { dependencyPrimitive(); primitive(); localValue(); relayValue(); recovery(); defaultValue() }\n");
        write_manifest_cone(&root, "dev.example", name, "executable", &source);
        let manifest =
            write_dependency_manifest(&root, name, &[&provider_coordinate, &first_coordinate])
                .replace("kind = \"library\"", "kind = \"executable\"");
        std::fs::write(root.join("Cone.toml"), manifest).unwrap();
        let consumer = build_manifest_request(
            workspace.path(),
            &target,
            &root,
            &workspace.path().join(format!("{name}.slib")),
            vec![
                provider.artifact().path().to_owned(),
                first.artifact().path().to_owned(),
            ],
            Vec::new(),
        )
        .with_debug_info(debug_info)
        .build_and_publish()
        .unwrap();
        let identity = consumer
            .artifact()
            .summary()
            .coordinate()
            .identity()
            .unwrap();
        let bytes = std::fs::read(consumer.artifact().path()).unwrap();
        let closure = scoop_slib::read_cross_cone_layout_artifact_closure(
            scoop_slib::CrossConeArtifactClosureInput::completed(
                identity,
                target.lir_target_selection(),
                consumer
                    .artifact()
                    .summary()
                    .direct_dependencies()
                    .iter()
                    .map(|dependency| dependency.identity())
                    .collect(),
                dependency_bytes.iter().map(Vec::as_slice).collect(),
                &bytes,
            ),
        )
        .unwrap();
        let (artifact, current) = closure.artifact(identity).unwrap();
        let (_, first) = closure
            .artifact(first_coordinate.identity().unwrap())
            .unwrap();
        let required_bodies = current
            .odr_members()
            .groups()
            .iter()
            .flat_map(|group| group.members())
            .filter(|member| member.role() == OdrMemberRole::CallableBody)
            .map(|member| member.member())
            .collect::<BTreeSet<_>>();
        let current_definitions = current
            .defined_symbols()
            .owners()
            .iter()
            .map(|definition| definition.owner())
            .collect::<BTreeSet<_>>();
        for member in current
            .odr_members()
            .groups()
            .iter()
            .flat_map(|group| group.members())
        {
            if member.role() == OdrMemberRole::RegistrationRecord {
                assert!(
                    current_definitions
                        .contains(&LinkDefinitionOwnerV1::OdrDefinition(member.member())),
                    "registration records are local definitions, never independent imports"
                );
            }
        }
        let reused_bodies = first.defined_symbols().owners().iter().filter(|definition| {
            matches!(definition.owner(), LinkDefinitionOwnerV1::OdrDefinition(member) if required_bodies.contains(&member))
                && !current_definitions.contains(&definition.owner())
        }).collect::<Vec<_>>();
        match debug_info {
            DebugInfoLevel::None => {
                assert!(
                    !reused_bodies.is_empty(),
                    "the consumer must import an existing generic body"
                );
                for body in reused_bodies {
                    for definition in first
                        .defined_symbols()
                        .owners()
                        .iter()
                        .filter(|definition| definition.member() == body.member())
                    {
                        assert!(
                            !current_definitions.contains(&definition.owner()),
                            "private records stay with their reused body"
                        );
                    }
                }
                let inputs = closure.dependency_first().map(|(artifact, symbols)| {
                    (
                        artifact.identity_graph(),
                        symbols.defined_symbols(),
                        symbols.odr_members(),
                        if artifact.identity() == identity {
                            &[]
                        } else {
                            artifact.manifest().direct_dependencies()
                        },
                    )
                });
                assert!(
                    matches!(
                        scoop_slib::merge_cross_cone_odr_definitions(inputs),
                        Err(scoop_slib::OdrDefinitionMergeError::MissingPhysicalDefinition { provider, .. }) if provider == identity
                    ),
                    "an unrelated artifact cannot satisfy an ODR import"
                );
            }
            DebugInfoLevel::Full => assert!(
                reused_bodies.is_empty(),
                "debug bodies must retain their own source information"
            ),
            DebugInfoLevel::LineTables => unreachable!("the test selects none and full"),
        }
        let physical = artifact
            .manifest()
            .members()
            .iter()
            .map(|member| member.id())
            .collect::<BTreeSet<_>>();
        assert!(
            current
                .defined_symbols()
                .owners()
                .iter()
                .all(|definition| physical.contains(&definition.member()))
        );
        let file =
            ArtifactFile::open(consumer.artifact().path(), target.lir_target_selection()).unwrap();
        let linked = scoop_slib::read_program_link_closure(
            &file,
            &dependency_files.iter().collect::<Vec<_>>(),
            target.lir_target_selection(),
        )
        .unwrap();
        assert_eq!(linked.artifacts().len(), 4);
    }
}

#[test]
fn removing_a_provider_instance_updates_the_existing_compile_dependency() {
    let target = resolved_target().expect("ODR availability requires a target");
    let workspace = tempfile::tempdir().unwrap();
    let core = bootstrap_core(workspace.path(), &target);
    let coordinate = ConeCoordinate::new("dev.example", "available-provider", "0.1.0").unwrap();
    let provider_root = workspace.path().join("provider");
    let source = "package dev.example\npublic fun <T> identity(value: T): T = value\npublic fun forced(): Int = identity(7)\n";
    write_manifest_cone(
        &provider_root,
        "dev.example",
        coordinate.name(),
        "library",
        source,
    );
    let provider = build_manifest(
        workspace.path(),
        &target,
        &provider_root,
        &workspace.path().join("provider.slib"),
    );
    let root = workspace.path().join("consumer");
    write_manifest_cone(
        &root,
        "dev.example",
        "availability-consumer",
        "executable",
        "package dev.example.consumer\nimport dev.example.identity\nimport dev.example.forced\nfun main() { identity(forced()) }\n",
    );
    let manifest = write_dependency_manifest(&root, "availability-consumer", &[&coordinate])
        .replace("kind = \"library\"", "kind = \"executable\"");
    std::fs::write(root.join("Cone.toml"), manifest).unwrap();
    let compile_consumer = || {
        build_manifest_request(
            workspace.path(),
            &target,
            &root,
            &workspace.path().join("consumer.slib"),
            vec![provider.artifact().path().to_owned()],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap()
    };
    let consumer = compile_consumer();
    let core_file =
        ArtifactFile::open(core.artifact().path(), target.lir_target_selection()).unwrap();
    std::fs::write(
        provider_root.join("src/main.scoop"),
        source.replace("identity(7)", "7"),
    )
    .unwrap();
    let updated = build_manifest(
        workspace.path(),
        &target,
        &provider_root,
        provider.artifact().path(),
    );
    assert_ne!(
        provider
            .artifact()
            .summary()
            .dependency_record()
            .lir_fingerprint(),
        updated
            .artifact()
            .summary()
            .dependency_record()
            .lir_fingerprint(),
        "removing a shared implementation must invalidate the existing Compile dependency",
    );
    let updated_file =
        ArtifactFile::open(updated.artifact().path(), target.lir_target_selection()).unwrap();
    let consumer_file =
        ArtifactFile::open(consumer.artifact().path(), target.lir_target_selection()).unwrap();
    assert!(
        scoop_slib::read_program_link_closure(
            &consumer_file,
            &[&core_file, &updated_file],
            target.lir_target_selection()
        )
        .is_err()
    );
    let rebuilt = compile_consumer();
    let rebuilt_file =
        ArtifactFile::open(rebuilt.artifact().path(), target.lir_target_selection()).unwrap();
    scoop_slib::read_program_link_closure(
        &rebuilt_file,
        &[&core_file, &updated_file],
        target.lir_target_selection(),
    )
    .unwrap();
}
