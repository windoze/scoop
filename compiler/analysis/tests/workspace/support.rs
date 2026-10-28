use scoop_analysis::ProjectConfiguration;
use scoop_protocol::CompilationPurpose;
use std::path::{Path, PathBuf};

pub fn write(root: &Path, name: &str, text: &str) -> PathBuf {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

pub fn setup(root: &Path) -> ProjectConfiguration {
    write(
        root,
        "core/Cone.toml",
        "schema = 1\n[cone]\ngroup = 'scoop'\nname = 'scoop.core'\nversion = '0.1.0'\nkind = 'library'\n",
    );
    write(
        root,
        "core/src/core.scoop",
        "package scoop.core\npublic class Marker\n",
    );
    write(
        root,
        "app/Cone.toml",
        "schema = 1\n[cone]\ngroup = 'test'\nname = 'app'\nversion = '1.0.0'\nkind = 'library'\n[dependencies]\n'scoop:scoop.core' = {version = '0.1.0', path = '../core'}\n",
    );
    write(
        root,
        "app/src/value.scoop",
        "package test.app\nfun value(): Int = 42\n",
    );
    let configuration = ProjectConfiguration {
        name: "app".into(),
        root: root.join("app"),
        target: scoop_toolchain::select_target("aarch64-apple-darwin").unwrap(),
        purpose: CompilationPurpose::Production,
        sysroot: root.join("sysroot"),
        artifact_search_roots: Vec::new(),
        package_manager: std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!("scoop-pm{}", std::env::consts::EXE_SUFFIX)),
        package_cache: Some(root.join("pm-cache")),
        lockfile: None,
    };
    resolve(&configuration);
    configuration
}

pub fn resolve(configuration: &ProjectConfiguration) {
    let output = std::process::Command::new(&configuration.package_manager)
        .arg("resolve")
        .arg(&configuration.root)
        .arg("--offline")
        .arg("--target")
        .arg(configuration.target.target().contract().canonical_triple())
        .arg("--purpose")
        .arg(if configuration.purpose == CompilationPurpose::Test {
            "test"
        } else {
            "production"
        })
        .arg("--sysroot")
        .arg(&configuration.sysroot)
        .arg("--package-cache-dir")
        .arg(configuration.package_cache.as_ref().unwrap())
        .output()
        .expect("build the companion scoop-pm before project integration tests");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
