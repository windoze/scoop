use super::*;
use std::process::Command;

#[test]
fn artifact_cxx_requirement_rejects_both_musl_final_link_modes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    let core = path.join("core.slib");
    let source = path.join("source");
    let root = path.join("root.slib");
    std::fs::create_dir_all(source.join("src")).unwrap();
    std::fs::write(
        source.join("Cone.toml"),
        "schema = 1\n[cone]\ngroup = \"dev.m33\"\nname = \"artifact-cxx\"\n\
         version = \"0.1.0\"\nkind = \"executable\"\n[native]\ncxx = true\n",
    )
    .unwrap();
    std::fs::write(source.join("src/main.scoop"), "fun main() {}\n").unwrap();
    for (input, output, dependencies) in [
        (workspace.join("sysroot/lib/scoop.core"), &core, vec![]),
        (source.clone(), &root, vec![&core]),
    ] {
        let mut command = Command::new(&compiler);
        command
            .arg("build")
            .arg(input)
            .args(["--target", "x86_64-unknown-linux-gnu"])
            .arg("--out-slib")
            .arg(output);
        for dependency in dependencies {
            command.arg("--direct-slib").arg(dependency);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::remove_dir_all(source).unwrap();
    let gnu = ResolvedTargetProfile::resolve("x86_64-unknown-linux-gnu").unwrap();
    let closure = read_program_artifacts(
        &root,
        &[core],
        gnu.lir_target_selection(),
        gnu.c_bridge_toolchain().profile(),
    )
    .unwrap();
    let musl = ResolvedTargetProfile::resolve("x86_64-unknown-linux-musl").unwrap();
    for mode in [
        scoop_toolchain::LinkMode::Static,
        scoop_toolchain::LinkMode::Dynamic,
    ] {
        let options = FinalLinkOptions {
            mode: Some(mode),
            ..Default::default()
        };
        let message = resolve_program_link_profile(&closure, &musl, &options)
            .unwrap_err()
            .to_string();
        assert!(message.contains("C++ native runtime is not supported for Linux musl"));
        assert!(message.contains("dev.m33:artifact-cxx:0.1.0"), "{message}");
    }
}
