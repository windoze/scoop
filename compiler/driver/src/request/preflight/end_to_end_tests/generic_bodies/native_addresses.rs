use super::*;

#[test]
fn imported_native_addresses_use_definition_owned_storage_bridges() {
    super::members::check_fixture_cases(
        "m23-native-addresses",
        &[
            "standalone",
            "already-used",
            "defaults",
            "wide",
            "sink",
            "aliases",
        ],
        &[
            "bad-managed",
            "bad-signature",
            "bad-generic",
            "bad-unsafe",
            "bad-private",
        ],
        "downstream",
    );
    super::members::check_fixture_cases(
        "m23-native-addresses",
        &["aliases"],
        &[],
        "alias-downstream",
    );
}

#[test]
fn native_storage_exports_do_not_emit_unused_c_trampolines() {
    let target = resolved_target().expect("native callback publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let source = std::fs::read_to_string(
        crate::workspace_root().join("tests/fixtures/m23-native-addresses/provider.scoop"),
    )
    .unwrap();
    let root = sysroot.path().join("provider");
    write_manifest_cone(
        &root,
        "dev.example",
        "native-storage-provider",
        "library",
        &source,
    );
    let mut request = build_manifest_request(
        sysroot.path(),
        &target,
        &root,
        &sysroot.path().join("output/provider.slib"),
        vec![],
        vec![],
    );
    request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(StageDumpKind::Mir));
    let provider = request.build_and_publish().unwrap();
    assert_eq!(
        provider
            .emitted_dumps()
            .first()
            .unwrap()
            .text()
            .lines()
            .filter(|line| line.starts_with("  callback cb"))
            .count(),
        6,
        "only source functions have native storage; property accessors do not"
    );
    let identity = provider
        .artifact()
        .summary()
        .coordinate()
        .identity()
        .unwrap();
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let bytes = std::fs::read(provider.artifact().path()).unwrap();
    let closure = scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            identity,
            target.lir_target_selection(),
            vec![ConeIdentity::CORE],
            vec![&core_bytes],
            &bytes,
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    let (sections, link) = closure.artifact(identity).unwrap();
    let mut sources = Vec::new();
    for binding in sections.mir_type_bridge().exports().callables().entries() {
        if binding.lowering_role() != &scoop_mir::MirCallableLoweringRoleV1::StaticCallbackStorage {
            continue;
        }
        let scoop_mir::MirCallableOriginV1::Generated {
            role:
                scoop_identity::GeneratedCallableKey::StaticNoGcCallbackStorageBridge { source, .. },
            ..
        } = binding.origin()
        else {
            panic!("a native storage bridge retains its source");
        };
        let scoop_identity::CallableTemplateOwner::Function(function) = source.template() else {
            panic!("native storage belongs to an ordinary source function");
        };
        let key = sections
            .identity_graph()
            .canonical_key::<_, scoop_identity::SourceDeclarationKey>(function)
            .unwrap();
        let scoop_identity::DeclarationName::Named(name) = key.name() else {
            panic!("source callback name");
        };
        sources.push(name.as_str().to_owned());
        let abi = sections
            .lir_exports()
            .callables()
            .get(binding.implementation())
            .unwrap();
        assert_eq!(abi.physical_definition().provider(), identity);
        assert_eq!(
            abi.canonical_signature().signature(),
            binding.lowered_signature().exact()
        );
    }
    sources.sort();
    assert_eq!(
        sources,
        [
            "echo",
            "echo",
            "hidden",
            "previouslyUsed",
            "roundWide",
            "sink"
        ]
    );
    assert_eq!(
        sections
            .lir_strong_production()
            .generated_bridge_plan()
            .units()
            .len(),
        1,
        "only previouslyUsed has an actual address expression in the provider"
    );
    assert_eq!(link.object_contents().generated_objects().len(), 1);
}
