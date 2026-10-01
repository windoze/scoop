use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use scoop::{RuntimeBuildRequest, RuntimeOptimization, build_runtime};
use scoop_toolchain::ResolvedTargetProfile;

mod orchestration;

pub struct Environment {
    _directory: tempfile::TempDir,
    compiler: PathBuf,
    linker: PathBuf,
    core: PathBuf,
    runtime_index: PathBuf,
}

pub fn environment() -> &'static Environment {
    static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();
    ENVIRONMENT.get_or_init(|| {
        let directory = tempfile::tempdir().unwrap();
        let compiler = paired_binary("SCOOP_TEST_PAIRED_SCOOPC", "scoopc");
        let linker = paired_binary("SCOOP_TEST_PAIRED_SCOOP_LINK", "scoop-link");
        let sources = directory.path().join("sources");
        let runtime = sources.join("runtime");
        copy_tree(&workspace().join("runtime/src"), &runtime.join("src"));
        copy_tree(
            &workspace().join("runtime/include"),
            &runtime.join("include"),
        );
        let target = ResolvedTargetProfile::resolve("aarch64-apple-darwin").unwrap();
        let runtime_build = build_runtime(RuntimeBuildRequest {
            target: &target,
            runtime_root: &runtime,
            cache_root: &directory.path().join("cache"),
            optimization: RuntimeOptimization::Optimized,
        })
        .unwrap();
        let runtime_index = runtime_build.index().to_path_buf();
        let core_source = sources.join("core");
        copy_tree(&workspace().join("sysroot/lib/scoop.core"), &core_source);
        let core = directory.path().join("core.slib");
        checked(
            Command::new(&compiler)
                .arg("build")
                .arg(&core_source)
                .arg("--out-slib")
                .arg(&core),
        );
        std::fs::remove_dir_all(sources).unwrap();
        Environment {
            _directory: directory,
            compiler,
            linker,
            core,
            runtime_index,
        }
    })
}

impl Environment {
    pub fn build(
        &self,
        directory: &Path,
        name: &str,
        kind: &str,
        source: &str,
        dependencies: &[(&str, &Path)],
    ) -> PathBuf {
        let cone = directory.join("sources").join(name);
        std::fs::create_dir_all(cone.join("src")).unwrap();
        let mut manifest = format!(
            "schema = 1\n[cone]\ngroup = \"dev.programlink\"\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"{kind}\"\n"
        );
        if !dependencies.is_empty() {
            manifest.push_str("\n[dependencies]\n");
            for (name, _) in dependencies {
                manifest.push_str(&format!("\"dev.programlink:{name}\" = \"0.1.0\"\n"));
            }
        }
        std::fs::write(cone.join("Cone.toml"), manifest).unwrap();
        std::fs::write(cone.join("src/main.scoop"), source).unwrap();
        let artifact = directory.join(format!("{name}.slib"));
        let mut command = Command::new(&self.compiler);
        command
            .arg("build")
            .arg(cone)
            .arg("--direct-slib")
            .arg(&self.core);
        for (_, dependency) in dependencies {
            command.arg("--direct-slib").arg(dependency);
        }
        checked(command.arg("--out-slib").arg(&artifact));
        artifact
    }

    pub fn link(&self, root: &Path, dependencies: &[&Path], output: &Path) -> String {
        let mut command = Command::new(&self.linker);
        command
            .arg("--root-slib")
            .arg(root)
            .arg("--dep-slib")
            .arg(&self.core)
            .arg("--runtime-objects")
            .arg(&self.runtime_index)
            .args(["--target", "aarch64-apple-darwin", "--dump-plan"])
            .arg("-o")
            .arg(output)
            .env("PATH", "/usr/bin:/bin")
            .env("SCOOP_SYSROOT", "/absent/scoop-program-link-sysroot")
            .env("LLVM_CONFIG_PATH", "/absent/scoop-program-link-llvm-config")
            .env("LLVM_SYS_221_PREFIX", "/absent/scoop-program-link-llvm");
        for dependency in dependencies {
            command.arg("--dep-slib").arg(dependency);
        }
        String::from_utf8(checked(&mut command).stdout).unwrap()
    }
}

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        workspace()
            .join("tests/fixtures/m23-program-link")
            .join(name),
    )
    .unwrap()
}

pub fn run(path: &Path, stress: bool) -> String {
    let mut command = Command::new(path);
    command.env_remove("SCOOP_GC_STRESS_MOVE");
    if stress {
        command.env("SCOOP_GC_STRESS_MOVE", "1");
    }
    String::from_utf8(checked(&mut command).stdout).unwrap()
}

fn paired_binary(variable: &str, name: &str) -> PathBuf {
    let path = std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(name)
        });
    assert!(
        path.is_file(),
        "build the paired binaries first: cargo build -p scoopc -p scoop-linker --bins; missing {}",
        path.display()
    );
    path
}

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn checked(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &path);
        } else {
            std::fs::copy(entry.path(), path).unwrap();
        }
    }
}
