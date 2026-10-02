use super::*;

fn reject_source(
    environment: &Environment,
    directory: &Path,
    dependency: &Path,
    fixture: &str,
    token: &str,
    diagnostic: &str,
) {
    let source = native_fixture(fixture);
    let cone = directory.join("source-error");
    std::fs::create_dir_all(cone.join("src")).unwrap();
    std::fs::write(cone.join("Cone.toml"), "schema = 1\n[cone]\ngroup = \"dev.programlink\"\nname = \"source-error\"\nversion = \"0.1.0\"\nkind = \"executable\"\n[dependencies]\n\"dev.programlink:storage-provider\" = \"0.1.0\"\n").unwrap();
    std::fs::write(cone.join("src/main.scoop"), &source).unwrap();
    let output = Command::new(&environment.compiler)
        .arg("build")
        .arg(&cone)
        .arg("--direct-slib")
        .arg(&environment.core)
        .arg("--direct-slib")
        .arg(dependency)
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
            &provider,
            &format!("storage/{fixture}.scoop"),
            token,
            diagnostic,
        );
    }
}
