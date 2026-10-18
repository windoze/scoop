use std::path::Path;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
        .map(|value| value.trim().to_owned())
}

fn main() {
    let path = Path::new("../../scripts/release.toml");
    println!("cargo:rerun-if-changed={}", path.display());
    let config: toml::Value = std::fs::read_to_string(path)
        .expect("read release configuration")
        .parse()
        .expect("parse release configuration");
    let minimum_os = config["targets"]["aarch64-apple-darwin"]["minimum_os"]
        .as_str()
        .expect("Darwin deployment baseline");
    println!("cargo:rustc-env=SCOOP_MACOS_BASELINE={minimum_os}");
    for name in ["SCOOP_RELEASE_VERSION", "SCOOP_SOURCE_REVISION"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let version = std::env::var("SCOOP_RELEASE_VERSION").unwrap_or_else(|_| {
        config["version"]
            .as_str()
            .expect("release version")
            .to_owned()
    });
    let revision = std::env::var("SCOOP_SOURCE_REVISION")
        .ok()
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "revision not provided".into());
    for name in [
        Some("HEAD".into()),
        git(&["symbolic-ref", "-q", "HEAD"]),
        Some("packed-refs".into()),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(path) = git(&["rev-parse", "--git-path", &name]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    println!("cargo:rustc-env=SCOOP_BUILD_VERSION={version} ({revision})");
}
