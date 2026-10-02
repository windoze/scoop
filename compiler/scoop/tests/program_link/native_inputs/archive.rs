use super::*;

#[test]
fn archives_resolve_member_chains_and_back_edges_without_unused_effects() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "archive",
        "executable",
        &native_fixture("archive/root.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "archive",
        &[],
        &[],
        "archive/root",
    );
    let entry = compile_native(
        directory.path(),
        "entry",
        &native_fixture("archive/entry.c"),
        &[],
    );
    let end = compile_native(
        directory.path(),
        "end",
        &native_fixture("archive/end.c"),
        &[],
    );
    let unused = compile_native(
        directory.path(),
        "unused",
        &native_fixture("archive/unused.c"),
        &[],
    );
    let middle = compile_native(
        directory.path(),
        "middle",
        &native_fixture("archive/middle.c"),
        &[],
    );
    archive_native(directory.path(), "m23_chain", &[&entry, &end, &unused]);
    archive_native(directory.path(), "m23_tail", &[&middle]);
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    for object in [entry, end, unused, middle] {
        std::fs::remove_file(object).unwrap();
    }
    let (program, plan) = link_native(environment, &root, &[], directory.path());
    assert_eq!(plan.matches("selected=true").count(), 3, "{plan}");
    assert_eq!(plan.matches("selected=false").count(), 1, "{plan}");
    assert!(plan.contains("name=\"unused.o\""), "{plan}");
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "42\n7\n");
    }
    let symbols = checked(Command::new("/usr/bin/nm").arg(&program));
    assert!(
        !String::from_utf8(symbols.stdout)
            .unwrap()
            .contains("m23_unused")
    );
}

#[test]
fn archive_same_names_and_identical_members_keep_physical_identity() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "same",
        "executable",
        &native_fixture("direct/root.scoop"),
        &[],
    );
    let left = compile_native(
        &directory.path().join("left"),
        "same",
        &native_fixture("direct/native.c"),
        &[],
    );
    let right = directory.path().join("right/same.o");
    std::fs::create_dir_all(right.parent().unwrap()).unwrap();
    std::fs::copy(&left, &right).unwrap();
    archive_native(directory.path(), "m23_math", &[&left, &right]);
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, plan) = link_native(environment, &root, &[], directory.path());
    assert_eq!(plan.matches("name=\"same.o\"").count(), 2, "{plan}");
    assert_eq!(plan.matches("selected=true").count(), 1, "{plan}");
    assert_eq!(plan.matches("selected=false").count(), 1, "{plan}");
    assert!(plan.contains("member 0 header=8 "), "{plan}");
    assert!(plan.contains("member 1 header="), "{plan}");
    assert_eq!(run(&program, false), "42\n");
}

#[test]
fn archive_selected_member_conflicts_and_missing_helpers_report_chain() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "archive",
        "executable",
        &native_fixture("direct/root.scoop"),
        &[],
    );
    for (extra, expected) in [
        (
            "int missing(void); int helper(void) { return missing(); }",
            "reference chain: _missing",
        ),
        (
            "int m23_add(int x, int y) { return x + y; } int helper(void) { return 0; }",
            "conflicts with Native",
        ),
        (
            "__attribute__((weak)) int m23_add(int x, int y) { return x + y; } int helper(void) { return 0; }",
            "conflicts with Native",
        ),
        (
            "int helper(void) { return 0; } __attribute__((constructor)) static void setup(void) {}",
            "forbidden initialization",
        ),
    ] {
        let entry = compile_native(
            directory.path(),
            "first",
            "extern int helper(void); int m23_add(int x, int y) { return x + y + helper(); }",
            &[],
        );
        let extra = compile_native(directory.path(), "second", extra, &[]);
        let path = archive_native(directory.path(), "m23_math", &[&entry, &extra]);
        reject_native(
            environment,
            &root,
            &[],
            directory.path(),
            &[directory.path().join("native")],
            &[expected, "member-", "_helper"],
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn archive_symbol_table_offsets_and_member_definitions_are_checked() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "toc",
        "executable",
        &native_fixture("direct/root.scoop"),
        &[],
    );
    let object = compile_native(
        directory.path(),
        "a_long_native_member_name",
        &native_fixture("direct/native.c"),
        &[],
    );
    let archive = archive_native(directory.path(), "m23_math", &[&object]);
    checked(Command::new("/usr/bin/ranlib").arg(&archive));
    let (program, plan) = link_native(environment, &root, &[], directory.path());
    assert!(
        plan.contains("name=\"a_long_native_member_name.o\""),
        "{plan}"
    );
    assert_eq!(run(&program, false), "42\n");
    let mut bytes = std::fs::read(&archive).unwrap();
    let offset = bytes
        .windows(b"_m23_add\0".len())
        .position(|value| value == b"_m23_add\0")
        .unwrap();
    bytes[offset..offset + 8].copy_from_slice(b"_m23_bad");
    std::fs::write(&archive, bytes).unwrap();
    reject_native(
        environment,
        &root,
        &[],
        directory.path(),
        &[directory.path().join("native")],
        &["archive symbol table", "_m23_bad"],
    );
}
