use super::*;
use scoop_identity::{RequestedConeKind, SourceRole};
use scoop_protocol::CompilationPurpose;

fn test_prepare(root: &Path, workspace: &Path) -> PreparedBuildGraph {
    request_for_purpose(root, workspace, CompilationPurpose::Test)
        .with_purpose(CompilationPurpose::Test)
        .unwrap()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
}

#[test]
fn only_the_root_merges_test_sources_and_dependencies() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    write_core(&workspace.join("sysroot"));
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let root = workspace.join("root");
    let helper = workspace.join("helper");
    write_manifest(
        &root,
        "root",
        "[test-dependencies]\n\"test:helper\" = { version = \"1.0.0\", path = \"../helper\" }\n",
    );
    write_manifest(
        &helper,
        "helper",
        "[test-dependencies]\n\"test:missing\" = { version = \"1.0.0\", path = \"../missing\" }\n",
    );
    std::fs::create_dir(root.join("tests")).unwrap();
    std::fs::write(root.join("tests/case.scoop"), "fun check() {}\n").unwrap();
    std::fs::create_dir(helper.join("tests")).unwrap();
    std::fs::write(helper.join("tests/invalid.scoop"), [0xff]).unwrap();
    let production = prepare(&root, workspace).unwrap();
    let test = test_prepare(&root, workspace);
    let root_id = test.root_identity();
    let helper_id = ConeCoordinate::new("test", "helper", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    assert_eq!(production.node_count(), 2);
    assert_eq!(test.node_count(), 3);
    assert_eq!(test.root_kind(), RequestedConeKind::Executable);
    assert_eq!(
        test.source_input(helper_id).unwrap().requested_kind(),
        RequestedConeKind::Library
    );
    assert_eq!(
        test.source_input(root_id)
            .unwrap()
            .sources()
            .map(|source| source.role())
            .collect::<Vec<_>>(),
        [SourceRole::Production, SourceRole::Test]
    );
    assert_eq!(test.source_input(helper_id).unwrap().sources().count(), 1);
    assert_eq!(
        test.compilation_purpose(helper_id),
        CompilationPurpose::Production
    );
    assert_eq!(test.compilation_purpose(root_id), CompilationPurpose::Test);
    assert_eq!(
        test.compile_cache_key(ConeIdentity::CORE, &[]).unwrap(),
        production
            .compile_cache_key(ConeIdentity::CORE, &[])
            .unwrap()
    );
    let plan = test
        .child_invocation_plan(
            ConeIdentity::CORE,
            RequestCorrelationId::from_array([3; 16]),
            &[],
        )
        .unwrap();
    assert_eq!(
        plan.request().build().purpose(),
        CompilationPurpose::Production
    );
    assert_eq!(
        std::fs::read(
            test.source_input_path(root_id)
                .unwrap()
                .join("tests/case.scoop")
        )
        .unwrap(),
        b"fun check() {}\n"
    );
}

#[test]
fn test_source_bytes_and_selection_do_not_invalidate_production() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    write_core(&workspace.join("sysroot"));
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let root = workspace.join("sysroot/src/scoop.core");
    std::fs::create_dir(root.join("tests")).unwrap();
    std::fs::write(root.join("tests/case.scoop"), "fun first() {}\n").unwrap();
    let production = prepare(&root, workspace).unwrap();
    let test = test_prepare(&root, workspace);
    let key = test.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    let production_key = production
        .compile_cache_key(ConeIdentity::CORE, &[])
        .unwrap();
    assert_ne!(key, production_key);
    let plan = test
        .child_invocation_plan(
            ConeIdentity::CORE,
            RequestCorrelationId::from_array([4; 16]),
            &[],
        )
        .unwrap();
    assert_eq!(plan.request().build().purpose(), CompilationPurpose::Test);
    std::fs::write(root.join("tests/case.scoop"), "fun second() {}\n").unwrap();
    let changed = test_prepare(&root, workspace)
        .compile_cache_key(ConeIdentity::CORE, &[])
        .unwrap();
    assert_ne!(key, changed);
    assert_eq!(
        key,
        test.compile_cache_key(ConeIdentity::CORE, &[]).unwrap()
    );
    let manifest = root.join("Cone.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        format!("{original}\n[test]\nsources = [{{ path = \"tests\" }}]\n"),
    )
    .unwrap();
    let selected = test_prepare(&root, workspace)
        .compile_cache_key(ConeIdentity::CORE, &[])
        .unwrap();
    assert_ne!(selected, changed);
    assert_eq!(
        production_key,
        prepare(&root, workspace)
            .unwrap()
            .compile_cache_key(ConeIdentity::CORE, &[])
            .unwrap()
    );
}

#[test]
fn test_graph_rejects_single_file_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("single.scoop");
    std::fs::write(&source, "fun main() {}\n").unwrap();
    assert!(matches!(
        single_file_request(&source, temp.path()).with_purpose(CompilationPurpose::Test),
        Err(crate::BuildGraphRequestError::TestRequiresManifest)
    ));
}
