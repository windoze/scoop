use super::*;

#[test]
fn renamed_reexport_retains_the_public_name_and_actual_source() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let source = native_fixture("direct/root.scoop")
        .replace("m23_math", "m23_facade")
        .replace("name = \"m23_add\"", "name = \"m23_renamed\"");
    let root = environment.build(directory.path(), "renamed", "executable", &source, &[]);
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
        "int facade_marker(void) { return 0; }",
        "@rpath/libm23_facade.dylib",
        &[
            "-Wl,-reexport_library",
            leaf.to_str().unwrap(),
            "-Wl,-alias,_m23_add,_m23_renamed",
        ],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, plan) = link_native(environment, &root, &[], directory.path());
    assert!(
        plan.contains("source=\"@rpath/libm23_leaf.dylib\":_m23_add"),
        "{plan}"
    );
    assert_eq!(run(&program, false), "42\n");
}

#[test]
fn conflicting_install_names_and_unsupported_load_paths_are_input_errors() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "overlap",
        "executable",
        &native_fixture("dynamic/overlap.scoop"),
        &[],
    );
    let install = directory.path().join("native/shared.dylib");
    dylib(
        directory.path(),
        "m23_a",
        &native_fixture("dynamic/a.c"),
        install.to_str().unwrap(),
        &[],
    );
    dylib(
        directory.path(),
        "m23_b",
        &native_fixture("dynamic/b.c"),
        install.to_str().unwrap(),
        &[],
    );
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native")],
        &["conflicting native providers for install name"],
    );
    dylib(
        directory.path(),
        "m23_a",
        &native_fixture("dynamic/a.c"),
        "@executable_path/libm23_a.dylib",
        &[],
    );
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native")],
        &["unsupported install name @executable_path"],
    );
}

#[test]
fn default_namespace_reports_multiple_visible_dynamic_providers() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let source = native_fixture("dynamic/overlap.scoop").replace(
        "fun main()",
        "@Extern(name = \"m23_shared\")\nfun shared(): Int\n\nfun main()",
    );
    let root = environment.build(directory.path(), "default", "executable", &source, &[]);
    dylib(
        directory.path(),
        "m23_a",
        &(native_fixture("dynamic/a.c") + "\nint m23_shared(void) { return 1; }\n"),
        "@rpath/libm23_a.dylib",
        &[],
    );
    dylib(
        directory.path(),
        "m23_b",
        &(native_fixture("dynamic/b.c") + "\nint m23_shared(void) { return 2; }\n"),
        "@rpath/libm23_b.dylib",
        &[],
    );
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native")],
        &[
            "ambiguous native providers for _m23_shared",
            "dev.programlink:default",
        ],
    );
}

#[test]
fn loader_relative_dependency_uses_the_original_provider_directory() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let source = native_fixture("direct/root.scoop").replace("m23_math", "m23_facade");
    let root = environment.build(directory.path(), "loader", "executable", &source, &[]);
    let leaf = dylib(
        directory.path(),
        "m23_leaf",
        &native_fixture("direct/native.c"),
        "@loader_path/private/libm23_leaf.dylib",
        &[],
    );
    dylib(
        directory.path(),
        "m23_facade",
        "extern int m23_add(int, int); int m23_forward(int x, int y) { return m23_add(x,y); }",
        "@rpath/libm23_facade.dylib",
        &["-Wl,-reexport_library", leaf.to_str().unwrap()],
    );
    let private = directory.path().join("native/private");
    std::fs::create_dir(&private).unwrap();
    checked(
        Command::new("/usr/bin/install_name_tool")
            .args(["-id", "@rpath/libm23_leaf.dylib"])
            .arg(&leaf),
    );
    std::fs::rename(leaf, private.join("libm23_leaf.dylib")).unwrap();
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, plan) = link_native(environment, &root, &[], directory.path());
    assert!(
        plan.contains("@loader_path/private/libm23_leaf.dylib"),
        "{plan}"
    );
    assert_eq!(run(&program, false), "42\n");
}
