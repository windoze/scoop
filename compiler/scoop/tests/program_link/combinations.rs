use super::*;

#[test]
fn cross_cone_value_storage_survives_boxing_arrays_and_moving_gc() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let library = environment.build(
        directory.path(),
        "values",
        "library",
        &fixture("values-library.scoop"),
        &[],
    );
    let root = environment.build(
        directory.path(),
        "root",
        "executable",
        &fixture("values-root.scoop"),
        &[("values", &library)],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let program = directory.path().join("program");
    environment.link(&root, &[&library], &program);
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "left\nright\n42\n42\ntrue\n");
    }
}

#[test]
fn coroutine_frames_helpers_closures_and_finally_are_consumed_from_artifacts() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let provider = environment.build(
        directory.path(),
        "coroutines",
        "library",
        &fixture("coroutine-provider.scoop"),
        &[],
    );
    for case in [
        "closure-finally",
        "failure-finally",
        "function-values",
        "lifecycle",
        "value-task",
    ] {
        let root = environment.build(
            directory.path(),
            case,
            "executable",
            &fixture(&format!("coroutine-{case}.scoop")),
            &[("coroutines", &provider)],
        );
        std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
        let program = directory.path().join(case);
        environment.link(&root, &[&provider], &program);
        for stress in [false, true] {
            assert_eq!(run(&program, stress), "42\n", "{case}");
        }
    }
}

#[test]
fn odr_member_unions_keep_independently_demanded_function_adapters() {
    let environment = environment();
    for family in ["adapters", "adapters-combined"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let provider = environment.build(
            path,
            "provider",
            "library",
            &fixture("union-provider.scoop"),
            &[],
        );
        let left = environment.build(
            path,
            "left",
            "library",
            &fixture(&format!("union-{family}-left.scoop")),
            &[("provider", &provider)],
        );
        let right = environment.build(
            path,
            "right",
            "library",
            &fixture(&format!("union-{family}-right.scoop")),
            &[("provider", &provider)],
        );
        let root = environment.build(
            path,
            "root",
            "executable",
            &fixture(&format!("union-{family}-consumer.scoop")),
            &[("provider", &provider), ("left", &left), ("right", &right)],
        );
        std::fs::remove_dir_all(path.join("sources")).unwrap();
        let program = path.join("program");
        environment.link(&root, &[&right, &left, &provider], &program);
        for stress in [false, true] {
            assert_eq!(run(&program, stress), "42\n", "{family}");
        }
    }
}

#[test]
fn rebuilt_core_protocol_selects_its_real_string_descriptor_beside_an_ordinary_string() {
    let original = environment();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("core-source");
    copy_tree(&workspace().join("sysroot/lib/scoop.core"), &source);
    let types = source.join("src/types.scoop");
    let original_types = std::fs::read_to_string(&types).unwrap();
    let changed = original_types.replace(
        "public class String : ToString, Hash {",
        "public class String : ToString, Hash {\n    public fun rebuiltAnswer(): Int = 42\n",
    );
    assert_ne!(changed, original_types);
    std::fs::write(types, changed).unwrap();
    std::fs::write(
        source.join("src/rebuilt-native.scoop"),
        fixture("rebuilt-core-native.scoop"),
    )
    .unwrap();
    let core = directory.path().join("rebuilt.slib");
    checked(
        Command::new(&original.compiler)
            .arg("build")
            .arg(&source)
            .arg("--out-slib")
            .arg(&core),
    );
    std::fs::remove_dir_all(source).unwrap();
    let environment = Environment {
        _directory: directory,
        core,
        compiler: original.compiler.clone(),
        linker: original.linker.clone(),
        runtime_index: original.runtime_index.clone(),
    };
    let path = environment._directory.path();
    let library = environment.build(
        path,
        "ordinary-string",
        "library",
        &fixture("shadow-string-library.scoop"),
        &[],
    );
    let root = environment.build(
        path,
        "root",
        "executable",
        &fixture("rebuilt-core-root.scoop"),
        &[("ordinary-string", &library)],
    );
    std::fs::remove_dir_all(path.join("sources")).unwrap();
    let program = path.join("program");
    environment.link(&root, &[&library], &program);
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "rebuilt\nroot\n42\n42\n");
    }
}
