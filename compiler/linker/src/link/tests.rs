use super::*;
use crate::test_support;

#[test]
fn native_alias_link_tracks_real_input_updates_and_missing_input() {
    let fixture = test_support::native_fixture();
    let directory = fixture.directory.path();
    let object = directory.join("m23_final.o");
    let alias_directory = directory.join("alias");
    std::fs::create_dir(&alias_directory).unwrap();
    let alias = alias_directory.join("m23_final.o");
    std::os::unix::fs::symlink(&object, &alias).unwrap();
    let quoted_directory = directory.join("objects with spaces, 'single' and \"double\" \\");
    std::fs::create_dir(&quoted_directory).unwrap();
    let output = link_program(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        std::slice::from_ref(&alias_directory),
        &quoted_directory.join("program"),
        &quoted_directory.join("cache"),
    )
    .unwrap();
    assert_eq!(output.fingerprint, fixture.output.fingerprint);
    assert_eq!(output.plan_dump, fixture.output.plan_dump);
    assert_eq!(run(&output.path), b"42\n");

    let replacement = directory.join("replacement.c");
    std::fs::write(
        &replacement,
        "int m23_final(int value) { return value + 80; }\n",
    )
    .unwrap();
    test_support::compile_native(&fixture.profile, &replacement, &object);

    let changed = link_program(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &fixture.library_paths,
        &directory.join("changed"),
        &quoted_directory.join("cache"),
    )
    .unwrap();
    assert_ne!(changed.fingerprint, output.fingerprint);
    assert_eq!(run(&changed.path), b"99\n");

    let old = std::fs::read(&output.path).unwrap();
    std::fs::remove_file(&alias).unwrap();
    let message = link_program(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &[alias_directory],
        &output.path,
        &quoted_directory.join("cache"),
    )
    .err()
    .unwrap()
    .to_string();
    assert!(message.contains("m23_final"), "{message}");
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
fn elf_shared_library_link_tracks_real_symbol_version_updates() {
    let fixture = test_support::native_fixture();
    let directory = fixture.directory.path();
    let libraries = directory.join("libraries");
    std::fs::create_dir(&libraries).unwrap();
    let library = libraries.join("libm23_final.so");
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
    let link = || {
        link_program(
            &fixture.closure,
            &fixture.runtime,
            &fixture.profile,
            std::slice::from_ref(&libraries),
            &directory.join("dso-program"),
            &directory.join("cache"),
        )
        .unwrap()
    };
    let first = link();
    assert_eq!(run(&first.path), b"42\n");
    compile("M28_2", 80);
    let second = link();
    assert_ne!(first.fingerprint, second.fingerprint);
    assert_eq!(run(&second.path), b"99\n");
}
