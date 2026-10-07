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

    let quoted_directory = directory.join("objects with spaces, 'single' and \"double\" \\");
    std::fs::create_dir(&quoted_directory).unwrap();
    let output = link_inputs(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &inputs,
        &quoted_directory.join("snapshot"),
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

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn elf_shared_library_link_uses_the_read_symbol_version_and_bytes() {
    let fixture = test_support::native_fixture();
    let directory = fixture.directory.path();
    std::fs::remove_file(directory.join("m23_final.o")).unwrap();
    let library = directory.join("libm23_final.so");
    let source = directory.join("shared.c");
    let script = directory.join("versions.map");
    let compile = |version: &str, offset: i32| {
        std::fs::write(
            &source,
            format!("int m23_final(int value) {{ return value + {offset}; }}\n"),
        )
        .unwrap();
        std::fs::write(
            &script,
            format!("{version} {{ global: m23_final; local: *; }};\n"),
        )
        .unwrap();
        let result = fixture
            .profile
            .startup_toolchain()
            .driver_command()
            .args(["-shared", "-fPIC", "-Wl,-soname,libm23_final.so"])
            .arg(format!("-Wl,--version-script={}", script.display()))
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .scoop_output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    compile("M28_1", 23);
    let original = std::fs::read(&library).unwrap();
    let inputs = ProgramInputs::new(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &fixture.library_paths,
    )
    .unwrap();
    compile("M28_2", 80);
    let output = link_inputs(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &inputs,
        &directory.join("dso-snapshot"),
    )
    .unwrap();
    // Loading follows the filesystem as usual; restore the version whose
    // immutable bytes were used by the linker, then execute the result.
    std::fs::write(&library, &original).unwrap();
    assert_eq!(run(&output.path), b"42\n");
    let old_output = std::fs::read(&output.path).unwrap();
    std::fs::write(&library, b"invalid ELF").unwrap();
    assert!(
        link_program(
            &fixture.closure,
            &fixture.runtime,
            &fixture.profile,
            &fixture.library_paths,
            &output.path
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&output.path).unwrap(), old_output);
}
