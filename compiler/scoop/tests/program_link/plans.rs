use super::*;

fn split_plan(output: &str) -> (&str, &str) {
    let (dump, fingerprint) = output.rsplit_once("link-plan ").unwrap();
    let fingerprint = fingerprint.trim();
    assert_eq!(fingerprint.len(), 64);
    assert!(fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()));
    (dump, fingerprint)
}

#[test]
fn effective_code_and_runtime_inputs_change_the_link_plan() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("program");
    let source = fixture("read-println.scoop");
    let root = environment.build(directory.path(), "root", "executable", &source, &[]);
    let original = environment.link(&root, &[], &output);
    assert_eq!(run(&output, false), "42\n");

    let changed_source = source.replace("42", "43");
    assert_ne!(changed_source, source);
    let root = environment.build(directory.path(), "root", "executable", &changed_source, &[]);
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let changed_code = environment.link(&root, &[], &output);
    assert_eq!(run(&output, false), "43\n");
    assert_eq!(split_plan(&original).0, split_plan(&changed_code).0);
    assert_ne!(split_plan(&original).1, split_plan(&changed_code).1);

    let runtime_directory = tempfile::tempdir().unwrap();
    let runtime_source = runtime_directory.path().join("runtime");
    copy_tree(
        &workspace().join("runtime/src"),
        &runtime_source.join("src"),
    );
    copy_tree(
        &workspace().join("runtime/include"),
        &runtime_source.join("include"),
    );
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let runtime = build_runtime(RuntimeBuildRequest {
        target: &target,
        runtime_root: &runtime_source,
        cache_root: &runtime_directory.path().join("cache"),
        optimization: RuntimeOptimization::None,
    })
    .unwrap();
    std::fs::remove_dir_all(runtime_source).unwrap();
    let changed_environment = Environment {
        _directory: runtime_directory,
        compiler: environment.compiler.clone(),
        linker: environment.linker.clone(),
        core: environment.core.clone(),
        runtime_index: runtime.index().to_path_buf(),
    };
    let changed_runtime = changed_environment.link(&root, &[], &output);
    assert_ne!(split_plan(&changed_code).1, split_plan(&changed_runtime).1);
    assert_eq!(run(&output, false), "43\n");
    assert_eq!(run(&output, true), "43\n");
}
