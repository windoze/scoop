use super::*;
use crate::test_support;

#[test]
fn native_file_and_symlink_replacement_cannot_change_the_read_link_inputs() {
    let fixture = test_support::native_fixture();
    let directory = fixture.directory.path();
    let object = directory.join("m23_final.o");
    let alias_directory = directory.join("alias");
    std::fs::create_dir(&alias_directory).unwrap();
    let alias = alias_directory.join("m23_final.o");
    std::os::unix::fs::symlink(&object, &alias).unwrap();
    let inputs = ProgramInputs::new(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        std::slice::from_ref(&alias_directory),
    )
    .unwrap();

    let replacement = directory.join("replacement.c");
    std::fs::write(
        &replacement,
        "int m23_final(int value) { return value + 80; }\n",
    )
    .unwrap();
    test_support::compile_native(&fixture.profile, &replacement, &object);
    std::fs::remove_file(replacement).unwrap();
    let invalid = directory.join("invalid.o");
    std::fs::write(&invalid, b"not an object").unwrap();
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&invalid, &alias).unwrap();

    let output = link_inputs(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &inputs,
        &directory.join("snapshot"),
    )
    .unwrap();
    assert_eq!(output.fingerprint, fixture.output.fingerprint);
    assert_eq!(output.plan_dump, fixture.output.plan_dump);
    assert_eq!(run(&output.path), b"42\n");

    let changed = link_program(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &fixture.library_paths,
        &directory.join("changed"),
    )
    .unwrap();
    assert_ne!(changed.fingerprint, output.fingerprint);
    assert_eq!(run(&changed.path), b"99\n");

    let old = std::fs::read(&output.path).unwrap();
    let message = link_program(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &[alias_directory],
        &output.path,
    )
    .err()
    .unwrap()
    .to_string();
    assert!(message.contains("native candidate"), "{message}");
    assert_eq!(std::fs::read(&output.path).unwrap(), old);
}

fn run(path: &Path) -> Vec<u8> {
    let output = std::process::Command::new(path)
        .env_clear()
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}
