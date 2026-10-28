use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use scoop_pm::lockfile::Lockfile;

pub fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub fn cone(name: &str, version: &str) -> String {
    format!(
        "schema = 1\n[cone]\ngroup = 'org.example'\nname = '{name}'\nversion = '{version}'\nkind = 'library'\n"
    )
}

pub fn core(root: &Path) {
    write(
        &root.join("core/Cone.toml"),
        "schema = 1\n[cone]\ngroup = 'scoop'\nname = 'scoop.core'\nversion = '0.1.0'\nkind = 'library'\n",
    );
}

pub fn app(root: &Path, name: &str, dependencies: &str) {
    write(
        &root.join(name).join("Cone.toml"),
        &format!(
            "{}\n[dependencies]\n{dependencies}\n[overrides]\n'scoop:scoop.core' = {{ path = '../core' }}\n",
            cone(name, "0.1.0")
        ),
    );
}

pub fn pm(root: &Path, directory: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_scoop-pm"));
    command
        .current_dir(directory)
        .args(args)
        .arg("--package-cache-dir")
        .arg(root.join("cache"))
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
        .env("GIT_CONFIG_VALUE_0", "always");
    for name in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        command.env_remove(name);
    }
    command.output().unwrap()
}

pub fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

pub fn failure(output: Output, expected: &str) {
    assert!(
        !output.status.success(),
        "unexpected success: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(expected), "expected {expected:?}: {error}");
}

pub fn lock(directory: &Path) -> Lockfile {
    Lockfile::read(&directory.join("Cone.lock"))
        .unwrap()
        .unwrap()
}
