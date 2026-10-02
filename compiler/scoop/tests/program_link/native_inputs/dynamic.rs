use super::*;
mod cases;

fn dylib(
    directory: &Path,
    name: &str,
    source: &str,
    install_name: &str,
    flags: &[&str],
) -> PathBuf {
    let object = compile_native(directory, &format!("build-{name}"), source, &[]);
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let toolchain = target.final_link().startup_toolchain();
    let path = directory.join("native").join(format!("lib{name}.dylib"));
    checked(
        Command::new(toolchain.compiler_driver())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-target", "arm64-apple-macos", "-dynamiclib", "-isysroot"])
            .arg(toolchain.sdk_root())
            .arg(format!(
                "-mmacosx-version-min={}",
                toolchain.profile().contract().deployment().minimum_os()
            ))
            .arg(&object)
            .args([
                "-install_name",
                install_name,
                "-current_version",
                "2.0",
                "-compatibility_version",
                "1.0",
            ])
            .args(flags)
            .arg("-o")
            .arg(&path),
    );
    path
}

#[test]
fn explicit_dynamic_bindings_choose_different_owners_for_overlapping_exports() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "dynamic",
        "executable",
        &native_fixture("dynamic/overlap.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "dynamic",
        &[],
        "dynamic/overlap",
    );
    dylib(
        directory.path(),
        "m23_a",
        &native_fixture("dynamic/a.c"),
        "@rpath/libm23_a.dylib",
        &[],
    );
    dylib(
        directory.path(),
        "m23_b",
        &native_fixture("dynamic/b.c"),
        "@rpath/libm23_b.dylib",
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, plan) = link_native(environment, &root, &[], directory.path());
    assert!(
        plan.contains("dynamic binding _m23_foo -> \"@rpath/libm23_a.dylib\""),
        "{plan}"
    );
    assert!(
        plan.contains("dynamic binding _m23_bar -> \"@rpath/libm23_b.dylib\""),
        "{plan}"
    );
    assert!(plan.contains("rpath "), "{plan}");
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "11\n22\n");
    }
}

#[test]
fn native_reexport_and_framework_keep_the_declared_library_owner() {
    let environment = environment();
    for framework in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let name = if framework { "Math" } else { "m23_facade" };
        let source = native_fixture("direct/root.scoop").replace("m23_math", name);
        let root = environment.build(directory.path(), "reexport", "executable", &source, &[]);
        let leaf = dylib(
            directory.path(),
            "m23_leaf",
            &native_fixture("direct/native.c"),
            "@rpath/libm23_leaf.dylib",
            &[],
        );
        let install = if framework {
            "@rpath/Math.framework/Math"
        } else {
            "@rpath/libm23_facade.dylib"
        };
        let library = dylib(
            directory.path(),
            name,
            "int facade_marker(void) { return 7; }",
            install,
            &["-Wl,-reexport_library", leaf.to_str().unwrap()],
        );
        if framework {
            let path = directory.path().join("native/Math.framework");
            std::fs::create_dir_all(&path).unwrap();
            std::fs::rename(library, path.join("Math")).unwrap();
        }
        std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
        let (program, plan) = link_native(environment, &root, &[], directory.path());
        assert!(
            plan.contains(&format!("dynamic binding _m23_add -> {install:?}")),
            "{plan}"
        );
        assert!(
            plan.contains("re-export \"@rpath/libm23_leaf.dylib\""),
            "{plan}"
        );
        assert_eq!(run(&program, false), "42\n");
    }
}

#[test]
fn ordinary_dynamic_dependency_does_not_become_a_public_export() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let source = native_fixture("direct/root.scoop").replace("m23_math", "m23_facade");
    let root = environment.build(
        directory.path(),
        "ordinary-load",
        "executable",
        &source,
        &[],
    );
    let leaf = dylib(
        directory.path(),
        "m23_leaf",
        &native_fixture("direct/native.c"),
        "@rpath/libm23_leaf.dylib",
        &[],
    );
    dylib(
        directory.path(),
        "m23_facade",
        "extern int m23_add(int, int); int facade_marker(void) { return m23_add(1, 2); }",
        "@rpath/libm23_facade.dylib",
        &[leaf.to_str().unwrap()],
    );
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native")],
        &["does not provide _m23_add"],
    );
    std::fs::remove_file(leaf).unwrap();
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native")],
        &[
            "dynamic dependency @rpath/libm23_leaf.dylib",
            "0 candidates",
        ],
    );
}

#[test]
fn native_absolute_install_name_and_standalone_text_stub_run() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "absolute",
        "executable",
        &native_fixture("direct/root.scoop"),
        &[],
    );
    let path = directory.path().join("native/libm23_math.dylib");
    dylib(
        directory.path(),
        "m23_math",
        &native_fixture("direct/native.c"),
        path.to_str().unwrap(),
        &[],
    );
    let (program, plan) = link_native(environment, &root, &[], directory.path());
    assert!(!plan.contains("\nrpath "), "{plan}");
    assert_eq!(run(&program, false), "42\n");
    let stub_root = directory.path().join("stubs");
    std::fs::create_dir(&stub_root).unwrap();
    let imports = std::collections::BTreeMap::from([(
        "_m23_add".into(),
        scoop_toolchain::NativeExport {
            kind: scoop_toolchain::SystemExportKind::Symbol,
            weak: false,
        },
    )]);
    std::fs::write(
        stub_root.join("libm23_math.tbd"),
        scoop_toolchain::write_link_stub(path.to_str().unwrap(), 2 << 16, 1 << 16, &imports)
            .unwrap(),
    )
    .unwrap();
    let program = directory.path().join("stub-program");
    checked(
        environment
            .link_command(&root, &[], &program)
            .arg("--library-path")
            .arg(&stub_root),
    );
    assert_eq!(run(&program, false), "42\n");
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native"), stub_root],
        &["ambiguous native library"],
    );
}
