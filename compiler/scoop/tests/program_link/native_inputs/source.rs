use super::*;

pub(super) fn reject_source(
    environment: &Environment,
    directory: &Path,
    dependencies: &[(&str, &Path)],
    fixture: &str,
    token: &str,
    diagnostic: &str,
) {
    let source = native_fixture(fixture);
    let cone = directory.join("source-error");
    std::fs::create_dir_all(cone.join("src")).unwrap();
    let mut manifest = String::from(
        "schema = 1\n[cone]\ngroup = \"dev.programlink\"\nname = \"source-error\"\nversion = \"0.1.0\"\nkind = \"executable\"\n[dependencies]\n",
    );
    for (name, _) in dependencies {
        manifest.push_str(&format!("\"dev.programlink:{name}\" = \"0.1.0\"\n"));
    }
    std::fs::write(cone.join("Cone.toml"), manifest).unwrap();
    std::fs::write(cone.join("src/main.scoop"), &source).unwrap();
    let mut command = Command::new(&environment.compiler);
    command
        .arg("build")
        .arg(&cone)
        .arg("--direct-slib")
        .arg(&environment.core);
    for (_, dependency) in dependencies {
        command.arg("--direct-slib").arg(dependency);
    }
    let output = command
        .arg("--out-slib")
        .arg(cone.join("rejected.slib"))
        .output()
        .unwrap();
    assert!(!output.status.success(), "{fixture}");
    let message = String::from_utf8(output.stderr).unwrap();
    let start = source.rfind(token).unwrap();
    assert!(
        message.contains(&format!("source 0:{start}..{}", start + token.len())),
        "{fixture}: {message}"
    );
    assert!(message.contains(diagnostic), "{fixture}: {message}");
    assert!(!cone.join("rejected.slib").exists());
}

#[test]
fn imported_native_storage_preserves_source_diagnostics() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let provider = environment.build(
        directory.path(),
        "storage-provider",
        "library",
        &native_fixture("storage/provider.scoop"),
        &[],
    );
    for (fixture, token, diagnostic) in [
        ("bad-unsafe", "counter", "reading an extern global"),
        (
            "bad-immutable",
            "limit",
            "cannot assign to immutable property `limit`",
        ),
        (
            "bad-type",
            "\"wrong\"",
            "cannot assign value of type String",
        ),
        (
            "bad-address",
            "counter",
            "dependency function `addressOf`: Int is not a subtype of Long",
        ),
    ] {
        reject_source(
            environment,
            directory.path(),
            &[("storage-provider", &provider)],
            &format!("storage/{fixture}.scoop"),
            token,
            diagnostic,
        );
    }
}
