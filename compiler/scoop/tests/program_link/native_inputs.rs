use super::*;

mod archive;
mod cabi;
mod callbacks;
mod contracts;
mod direct;
mod dynamic;
mod formats;
mod mixed;
mod resolution;
mod runtime;
mod scoopabi;
mod source;
mod storage;

fn archive_native(directory: &Path, name: &str, members: &[&Path]) -> PathBuf {
    let archive = directory.join("native").join(format!("lib{name}.a"));
    std::fs::create_dir_all(archive.parent().unwrap()).unwrap();
    checked(
        Command::new("/usr/bin/ar")
            .arg("qcS")
            .arg(&archive)
            .args(members),
    );
    archive
}

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

fn native_fixture(name: &str) -> String {
    std::fs::read_to_string(
        workspace()
            .join("tests/fixtures/m23-native-link")
            .join(name),
    )
    .unwrap()
}

fn compile_native(directory: &Path, name: &str, source: &str, flags: &[&str]) -> PathBuf {
    static TARGET: OnceLock<ResolvedTargetProfile> = OnceLock::new();
    let target = TARGET.get_or_init(|| ResolvedTargetProfile::resolve_host().unwrap());
    let toolchain = target.final_link().startup_toolchain();
    let source_path = directory.join("sources/native").join(format!("{name}.c"));
    std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    std::fs::write(&source_path, source).unwrap();
    let object = directory.join("native").join(format!("{name}.o"));
    std::fs::create_dir_all(object.parent().unwrap()).unwrap();
    checked(
        Command::new(toolchain.compiler_driver())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-target", "arm64-apple-macos", "-isysroot"])
            .arg(toolchain.sdk_root())
            .arg(format!(
                "-mmacosx-version-min={}",
                toolchain.profile().contract().deployment().minimum_os()
            ))
            .args(["-Wall", "-Wextra", "-Werror", "-fno-common", "-c"])
            .args(flags)
            .arg(&source_path)
            .arg("-o")
            .arg(&object),
    );
    object
}

fn link_native(
    environment: &Environment,
    root: &Path,
    deps: &[&Path],
    directory: &Path,
) -> (PathBuf, String) {
    let program = directory.join("program");
    let output = checked(
        environment
            .link_command(root, deps, &program)
            .arg("--library-path")
            .arg(directory.join("native")),
    );
    (program, String::from_utf8(output.stdout).unwrap())
}

fn reject_native(
    environment: &Environment,
    root: &Path,
    deps: &[&Path],
    directory: &Path,
    roots: &[PathBuf],
    expected: &[&str],
) {
    let program = directory.join("existing");
    std::fs::write(&program, b"previous executable").unwrap();
    let mut command = environment.link_command(root, deps, &program);
    for root in roots {
        command.arg("--library-path").arg(root);
    }
    let output = command.output().unwrap();
    assert!(!output.status.success(), "unexpected native link success");
    let message = String::from_utf8(output.stderr).unwrap();
    for expected in expected {
        assert!(message.contains(expected), "missing {expected}: {message}");
    }
    assert_eq!(std::fs::read(program).unwrap(), b"previous executable");
}

fn stage_snapshots(
    environment: &Environment,
    directory: &Path,
    name: &str,
    dependencies: &[&Path],
    support: &[&Path],
    fixture: &str,
) {
    for stage in ["hir", "mir", "lir"] {
        let mut command = Command::new(&environment.compiler);
        command
            .arg("build")
            .arg(directory.join("sources").join(name))
            .arg("--direct-slib")
            .arg(&environment.core);
        for dep in dependencies {
            command.arg("--direct-slib").arg(dep);
        }
        for dep in support {
            command.arg("--support-slib").arg(dep);
        }
        let output = checked(
            command
                .arg("--out-slib")
                .arg(directory.join(format!("{name}.slib")))
                .arg("--emit")
                .arg(stage),
        );
        let text = String::from_utf8(output.stdout).unwrap();
        let path = workspace()
            .join("tests/fixtures/m23-native-link")
            .join(format!("{fixture}.{stage}.snap"));
        if std::env::var("SCOOP_UPDATE_NATIVE_LINK_SNAPSHOTS").as_deref() == Ok("1") {
            std::fs::write(&path, &text).unwrap();
        }
        assert_eq!(
            text,
            std::fs::read_to_string(&path).unwrap(),
            "{}",
            path.display()
        );
    }
}
