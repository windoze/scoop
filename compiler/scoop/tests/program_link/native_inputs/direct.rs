use super::*;

#[test]
fn direct_candidates_merge_identical_bytes_and_reject_ambiguous_or_corrupt_inputs() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "direct",
        "executable",
        &native_fixture("direct/root.scoop"),
        &[],
    );
    let object = compile_native(
        directory.path(),
        "m23_math",
        &native_fixture("direct/native.c"),
        &[],
    );
    let alternate = directory.path().join("alternate");
    std::fs::create_dir(&alternate).unwrap();
    std::fs::copy(&object, alternate.join("m23_math.o")).unwrap();
    let (_, original) = link_native(environment, &root, &[], directory.path());
    let output = checked(
        environment
            .link_command(&root, &[], &directory.path().join("duplicate"))
            .arg("--library-path")
            .arg(&alternate)
            .arg("--library-path")
            .arg(directory.path().join("native")),
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), original);
    compile_native(
        directory.path(),
        "m23_math",
        &native_fixture("direct/native.c").replace("left + right", "left + right + 1"),
        &[],
    );
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[alternate.clone(), directory.path().join("native")],
        &[
            "ambiguous native library",
            "m23_math",
            "dev.programlink:direct",
        ],
    );
    std::fs::write(alternate.join("m23_math.o"), b"invalid Mach-O").unwrap();
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native"), alternate],
        &["native candidate", "m23_math.o"],
    );
}

#[test]
fn direct_object_checks_real_symbol_kind_target_and_effects() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "direct",
        "executable",
        &native_fixture("direct/root.scoop"),
        &[],
    );
    for (source, flags, expected) in [
        (
            "int m23_add = 42;",
            &[][..],
            "incompatible function/data/TLS/mutability",
        ),
        (
            "extern int missing_helper(void); int m23_add(int x, int y) { return x + y + missing_helper(); }",
            &[],
            "unresolved symbol _missing_helper",
        ),
        (
            "int m23_add(int x, int y) { return x + y; } __attribute__((constructor)) static void setup(void) {}",
            &[],
            "forbidden initialization",
        ),
        (
            "int m23_add;",
            &["-fcommon"][..],
            "common/tentative definition",
        ),
        (
            "int m23_add(int x, int y) { return x + y; }",
            &["-target", "x86_64-apple-macos"][..],
            "arm64 Mach-O",
        ),
    ] {
        compile_native(directory.path(), "m23_math", source, flags);
        reject_native(
            environment,
            &root,
            &[],
            directory.path(),
            &[directory.path().join("native")],
            &[expected],
        );
    }
}
