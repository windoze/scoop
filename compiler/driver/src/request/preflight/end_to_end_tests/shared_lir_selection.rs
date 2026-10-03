use super::*;

mod descriptors;
mod protocols;
mod selections;
type Compile<'closure, 'input> = scoop_slib::DirectCrossConeSemanticProvider<'closure>;

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
    let root = sysroot.path().join("combined");
    write_manifest_cone(
        &root,
        "dev.example",
        "projection-combined",
        "library",
        &fixture("combined"),
    );
    write_dependency_manifest(&root, "projection-combined", &[&provider_coordinate]);
    let request = build_manifest_request(
        sysroot.path(),
        &target,
        &root,
        &sysroot.path().join("output/combined.slib"),
        vec![provider.artifact().path().to_path_buf()],
        vec![],
    );
    let loaded = request.load_preflight().unwrap();
    let validated = loaded.validate().unwrap();
    let closure = &validated.dependencies().closure;
    let core = closure
        .semantic()
        .direct_provider(ConeIdentity::CORE)
        .unwrap();
    let ordinary = closure
        .semantic()
        .direct_provider(provider_coordinate.identity().unwrap())
        .unwrap();
    selections::check(closure.semantic(), &core, &ordinary);
    descriptors::check(closure.semantic(), &core, &ordinary);
    protocols::check(closure.semantic(), &core, &ordinary);
}
