use super::*;

mod descriptors;
mod protocols;
mod selections;
type Compile<'a> =
    scoop_slib::ValidatedCompileArtifact<'a, scoop_slib::CrossConeSemanticsStrongProfile>;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(crate::workspace_root().join(format!(
        "tests/fixtures/m23-shared-lir-selection/{name}.scoop"
    )))
    .unwrap()
}

#[test]
fn shared_protocol_and_lir_projection_uses_actual_providers() {
    let target = resolved_target().expect("shared LIR projection requires a host target");
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "projection-provider", "0.1.0").unwrap();
    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "projection-provider",
        "library",
        &fixture("provider"),
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("output/provider.slib"),
    );
    for name in ["standalone", "combined"] {
        let root = sysroot.path().join(name);
        let cone_name = format!("projection-{name}");
        write_manifest_cone(&root, "dev.example", &cone_name, "library", &fixture(name));
        let direct = if name == "combined" {
            write_dependency_manifest(&root, &cone_name, &[&provider_coordinate]);
            vec![provider.artifact().path().to_path_buf()]
        } else {
            vec![]
        };
        let output = sysroot.path().join(format!("output/{name}.slib"));
        let request = || {
            build_manifest_request(
                sysroot.path(),
                &target,
                &root,
                &output,
                direct.clone(),
                vec![],
            )
        };
        if name == "combined" {
            let loaded = request().load_preflight(DecodeLimits::default()).unwrap();
            let validated = loaded.validate().unwrap();
            let closure = &validated.dependencies().closure;
            let core = closure.share_artifact(ConeIdentity::CORE).unwrap();
            let ordinary = closure
                .share_artifact(provider_coordinate.identity().unwrap())
                .unwrap();
            selections::check(closure.semantic(), core.compile(), ordinary.compile());
            descriptors::check(closure.semantic(), core.compile(), ordinary.compile());
            protocols::check(closure.semantic(), core.compile(), ordinary.compile());
        }
        for (kind, suffix) in [(StageDumpKind::Mir, "mir"), (StageDumpKind::Lir, "lir")] {
            let mut request = request();
            request.emit = StageDumpPolicy::Stage(kind);
            let artifact = request.build_and_publish(DecodeLimits::default()).unwrap();
            let dump = artifact.emitted_dump().unwrap().text();
            let snapshot = crate::workspace_root().join(format!(
                "tests/fixtures/m23-shared-lir-selection/{name}.{suffix}.snap"
            ));
            if std::env::var_os("SCOOP_UPDATE_SHARED_LIR_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, dump).unwrap();
            }
            assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
        }
    }
}
