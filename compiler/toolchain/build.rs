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
    let catalog_path = Path::new("../target-data/targets.toml");
    println!("cargo:rerun-if-changed={}", catalog_path.display());
    let catalog: toml::Value = std::fs::read_to_string(catalog_path)
        .expect("read target catalog")
        .parse()
        .expect("parse target catalog");
    let mut baselines =
        String::from("pub(crate) const DARWIN_DEPLOYMENT_BASELINES: &[(&str, &str)] = &[\n");
    for target in catalog["targets"]
        .as_array()
        .expect("target catalog entries")
    {
        if target["native_environment"].as_str() == Some("Darwin") {
            let canonical = target["canonical"].as_str().expect("canonical target");
            let minimum_os = config["targets"][canonical]["minimum_os"]
                .as_str()
                .expect("Darwin deployment baseline");
            baselines.push_str(&format!("({canonical:?}, {minimum_os:?}),\n"));
        }
    }
    baselines.push_str("];\n");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("build output"));
    std::fs::write(output.join("deployment.rs"), baselines).expect("write deployment baselines");
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
