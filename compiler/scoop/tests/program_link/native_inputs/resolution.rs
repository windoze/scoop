use super::*;

#[test]
fn later_archive_selection_cannot_take_over_an_explicit_dynamic_binding() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "late",
        "executable",
        &native_fixture("resolution/root.scoop"),
        &[],
    );
    dylib(
        directory.path(),
        "m23_late_dynamic",
        &native_fixture("resolution/dynamic.c"),
        "@rpath/libm23_late_dynamic.dylib",
        &[],
    );
    let member = compile_native(
        directory.path(),
        "archive",
        &native_fixture("resolution/archive.c"),
        &[],
    );
    archive_native(directory.path(), "m23_late_archive", &[&member]);
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native")],
        &[
            "cannot bind _m23_a, already defined by Native",
            "dev.programlink:late",
        ],
    );
}

#[test]
fn default_namespace_does_not_scan_files_from_library_paths() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "default",
        "executable",
        &native_fixture("resolution/default.scoop"),
        &[],
    );
    compile_native(
        directory.path(),
        "m23_math",
        &native_fixture("direct/native.c"),
        &[],
    );
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native")],
        &[
            "unresolved native symbol _m23_add in default namespace",
            "dev.programlink:default",
        ],
    );
}

#[test]
fn native_runtime_location_and_install_name_change_the_link_plan() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "paths",
        "executable",
        &native_fixture("direct/root.scoop"),
        &[],
    );
    let library = dylib(
        directory.path(),
        "m23_math",
        &native_fixture("direct/native.c"),
        "@rpath/libm23_math.dylib",
        &["-Wl,-headerpad_max_install_names"],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, original) = link_native(environment, &root, &[], directory.path());
    assert_eq!(run(&program, false), "42\n");
    let moved = directory.path().join("moved");
    std::fs::create_dir_all(moved.join("native")).unwrap();
    let copy = moved.join("native/libm23_math.dylib");
    std::fs::copy(library, &copy).unwrap();
    let (program, relocated) = link_native(environment, &root, &[], &moved);
    assert_ne!(original, relocated);
    assert!(relocated.contains("moved/native"), "{relocated}");
    assert_eq!(run(&program, false), "42\n");
    checked(
        Command::new("/usr/bin/install_name_tool")
            .arg("-id")
            .arg(&copy)
            .arg(&copy),
    );
    let (program, renamed) = link_native(environment, &root, &[], &moved);
    assert_ne!(renamed, relocated);
    assert!(!renamed.contains("rpath "), "{renamed}");
    assert_eq!(run(&program, false), "42\n");
}
