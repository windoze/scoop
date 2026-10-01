use std::path::{Path, PathBuf};
use std::process::Command;

use scoop::{RuntimeBuildRequest, RuntimeOptimization, build_runtime};
use scoop_toolchain::{ResolvedTargetProfile, ValidatedFinalLinkProfile};

use crate::{ProgramLinkOutput, RuntimeObjectSet, link_program, read_program_artifacts};

pub(super) struct Fixture {
    pub directory: tempfile::TempDir,
    pub profile: ValidatedFinalLinkProfile,
    pub closure: scoop_slib::ProgramLinkClosure,
    pub runtime: RuntimeObjectSet,
    pub output: ProgramLinkOutput,
}

pub(super) fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let compiler = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("scoopc")
        });
    assert!(
        compiler.is_file(),
        "build the paired scoopc before program-link tests"
    );
    let core = path.join("core.slib");
    checked(
        Command::new(&compiler)
            .arg("build")
            .arg(workspace.join("sysroot/lib/scoop.core"))
            .arg("--out-slib")
            .arg(&core),
    );
    let library = compile(
        &compiler,
        path,
        "library",
        "library",
        &core,
        &[],
        "basic-library.scoop",
    );
    let root = compile(
        &compiler,
        path,
        "root",
        "executable",
        &core,
        &[&library],
        "basic-root.scoop",
    );
    let target = ResolvedTargetProfile::resolve("aarch64-apple-darwin").unwrap();
    let runtime_build = build_runtime(RuntimeBuildRequest {
        target: &target,
        runtime_root: &workspace.join("runtime"),
        cache_root: &path.join("runtime"),
        optimization: RuntimeOptimization::Optimized,
    })
    .unwrap();
    let profile = ValidatedFinalLinkProfile::resolve("aarch64-apple-darwin").unwrap();
    let runtime = RuntimeObjectSet::read_index(
        runtime_build.index(),
        profile.target(),
        profile.startup_toolchain().profile(),
    )
    .unwrap();
    let closure = read_program_artifacts(&root, &[library, core], &profile).unwrap();
    let output = link_program(&closure, &runtime, &profile, &path.join("program")).unwrap();
    Fixture {
        directory,
        profile,
        closure,
        runtime,
        output,
    }
}

fn compile(
    compiler: &Path,
    path: &Path,
    name: &str,
    kind: &str,
    core: &Path,
    dependencies: &[&Path],
    fixture: &str,
) -> PathBuf {
    let source = path.join(name);
    std::fs::create_dir_all(source.join("src")).unwrap();
    let mut manifest = format!(
        "schema = 1\n[cone]\ngroup = \"dev.programlink\"\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"{kind}\"\n"
    );
    if !dependencies.is_empty() {
        manifest.push_str("[dependencies]\n\"dev.programlink:library\" = \"0.1.0\"\n");
    }
    std::fs::write(source.join("Cone.toml"), manifest).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-program-link")
            .join(fixture),
        source.join("src/main.scoop"),
    )
    .unwrap();
    let output = path.join(format!("{name}.slib"));
    let mut command = Command::new(compiler);
    command
        .arg("build")
        .arg(&source)
        .arg("--direct-slib")
        .arg(core);
    for dependency in dependencies {
        command.arg("--direct-slib").arg(dependency);
    }
    checked(command.arg("--out-slib").arg(&output));
    std::fs::remove_dir_all(source).unwrap();
    output
}

fn checked(command: &mut Command) {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
