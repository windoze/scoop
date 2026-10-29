use std::path::{Path, PathBuf};
use std::process::Command;

use scoop_toolchain::{ResolvedTargetProfile, ValidatedFinalLinkProfile};

use crate::{ProgramLinkOutput, RuntimeObjectSet, link_program, read_program_artifacts};

pub(crate) struct Fixture {
    pub directory: tempfile::TempDir,
    pub profile: ValidatedFinalLinkProfile,
    pub closure: scoop_slib::ProgramLinkClosure,
    pub runtime: RuntimeObjectSet,
    pub output: ProgramLinkOutput,
    pub library_paths: Vec<PathBuf>,
}

pub(crate) fn native_fixture() -> Fixture {
    build_fixture("../native-link/final/root.scoop", true)
}

fn build_fixture(root_fixture: &str, native: bool) -> Fixture {
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
    let triple = if cfg!(target_os = "linux") {
        "x86_64-unknown-linux-gnu"
    } else {
        "aarch64-apple-darwin"
    };
    let target = ResolvedTargetProfile::resolve(triple).unwrap();
    let sysroot = std::env::var_os("SCOOP_SYSROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("sysroot"));
    let core =
        scoop_toolchain::TrustedCoreSlotLayoutV1::new(&sysroot, target.lir_target_selection())
            .artifact()
            .to_owned();
    let library = compile(
        &compiler,
        path,
        "library",
        "library",
        &sysroot,
        false,
        "basic-library.scoop",
    );
    let root = compile(
        &compiler,
        path,
        "root",
        "executable",
        &sysroot,
        true,
        root_fixture,
    );
    let profile = ValidatedFinalLinkProfile::resolve(triple).unwrap();
    let runtime = RuntimeObjectSet::read_index(
        &scoop_toolchain::default_runtime_index(&sysroot, target.lir_target(), false, false),
        profile.target(),
        profile.startup_toolchain().profile(),
    )
    .unwrap();
    let library_paths = if native {
        let source = path.join("native.c");
        std::fs::copy(
            workspace.join("tests/fixtures/language/native-link/final/native.c"),
            &source,
        )
        .unwrap();
        compile_native(&profile, &source, &path.join("m23_final.o"));
        vec![path.to_owned()]
    } else {
        Vec::new()
    };
    let closure = read_program_artifacts(
        &root,
        &[library, core],
        scoop_target_data::selection(profile.id())
            .expect("implemented target selected by this consumer"),
    )
    .unwrap();
    let output = link_program(
        &closure,
        &runtime,
        &profile,
        crate::ProgramLinkOptions {
            keep_symbols: true,
            emit_debug_file: false,
        },
        &library_paths,
        &path.join("program"),
        &path.join("cache"),
    )
    .unwrap();
    Fixture {
        directory,
        profile,
        closure,
        runtime,
        output,
        library_paths,
    }
}

fn compile(
    compiler: &Path,
    path: &Path,
    name: &str,
    kind: &str,
    sysroot: &Path,
    has_library: bool,
    fixture: &str,
) -> PathBuf {
    let source = path.join(name);
    std::fs::create_dir_all(source.join("src")).unwrap();
    let mut manifest = format!(
        "schema = 1\n[cone]\ngroup = \"dev.programlink\"\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"{kind}\"\n"
    );
    if has_library {
        manifest.push_str("[dependencies]\n\"dev.programlink:library\" = { version = \"0.1.0\", path = \"../library\" }\n");
    }
    std::fs::write(source.join("Cone.toml"), manifest).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/language/program-link")
            .join(fixture),
        source.join("src/main.scoop"),
    )
    .unwrap();
    let output = path.join(format!("{name}.slib"));
    checked(
        Command::new(compiler.with_file_name("scoop"))
            .arg("build")
            .arg(&source)
            .args(["--release", "--emit-slib", "--sysroot"])
            .arg(sysroot)
            .arg("--scoopc")
            .arg(compiler)
            .arg("--cache-dir")
            .arg(path.join("compile-cache"))
            .arg("-o")
            .arg(&output),
    );
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

pub(crate) fn compile_native(profile: &ValidatedFinalLinkProfile, source: &Path, output: &Path) {
    checked(
        &mut profile
            .startup_toolchain()
            .object_compilation_command(source, output),
    );
}
