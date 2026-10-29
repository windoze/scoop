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
    let gnu = ResolvedTargetProfile::resolve("x86_64-unknown-linux-gnu").unwrap();
    let sysroot = std::env::var_os("SCOOP_SYSROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("sysroot"));
    let core = scoop_toolchain::TrustedCoreSlotLayoutV1::new(&sysroot, gnu.lir_target_selection())
        .artifact()
        .to_owned();
    let source = path.join("source");
    let root = path.join("root.slib");
    std::fs::create_dir_all(source.join("src")).unwrap();
    std::fs::write(
        source.join("Cone.toml"),
        "schema = 1\n[cone]\ngroup = \"dev.m33\"\nname = \"artifact-cxx\"\n\
         version = \"0.1.0\"\nkind = \"executable\"\n[native]\ncxx = true\n",
    )
    .unwrap();
    std::fs::write(
        source.join("src/main.scoop"),
        "package dev.m33\n\nfun main() {}\n",
    )
    .unwrap();
    let output = Command::new(compiler.with_file_name("scoop"))
        .arg("build")
        .arg(&source)
        .args([
            "--release",
            "--emit-slib",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--sysroot",
        ])
        .arg(&sysroot)
        .arg("--scoopc")
        .arg(&compiler)
        .arg("--cache-dir")
        .arg(path.join("compile-cache"))
        .arg("-o")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let closure = read_program_artifacts(&root, &[core], gnu.lir_target_selection()).unwrap();
    let musl = ResolvedTargetProfile::resolve("x86_64-unknown-linux-musl").unwrap();
    for mode in [
        scoop_toolchain::LinkMode::Static,
        scoop_toolchain::LinkMode::Dynamic,
    ] {
        let options = FinalLinkOptions {
            mode: Some(mode),
            ..Default::default()
        };
        let message = resolve_program_link_profile(&closure, &musl, &options, None)
            .unwrap_err()
            .to_string();
        assert!(message.contains("C++ native runtime is not supported for Linux musl"));
        assert!(message.contains("dev.m33:artifact-cxx:0.1.0"), "{message}");
    }
}
