use std::{collections::BTreeSet, env, fmt::Write, fs, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    targets: Vec<Target>,
    hosts: Vec<Host>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    symbol: String,
    canonical: String,
    profile: String,
    version: u32,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    versioned_aliases: Vec<String>,
    os: String,
    arch: String,
    env: String,
    llvm_triple: String,
    layout: String,
    backend: String,
    procedure_abi: String,
    scoop_abi: String,
    exception_handling: String,
    tuning: String,
    runtime: String,
    native_environment: String,
    c_bridge_profile: String,
    native_rust_target: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    os: String,
    arch: String,
    environment: String,
    system: String,
    machines: Vec<String>,
    rust_target: String,
    default_target: String,
    distribution_targets: Vec<String>,
}

fn identifier(value: &str) {
    assert!(
        value.starts_with(|c: char| c.is_ascii_alphabetic())
            && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "invalid implementation identifier {value:?}"
    );
}

fn main() {
    println!("cargo:rerun-if-changed=targets.toml");
    let catalog: Catalog = toml::from_str(&fs::read_to_string("targets.toml").unwrap()).unwrap();
    assert!(!catalog.targets.is_empty(), "target catalog is empty");
    let mut names = BTreeSet::new();
    let mut profiles = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    let mut output = String::new();
    let mut source_key = String::new();
    for target in &catalog.targets {
        for name in std::iter::once(&target.canonical).chain(&target.aliases) {
            assert!(names.insert(name), "duplicate target spelling {name}");
        }
        assert!(
            profiles.insert((&target.profile, target.version)),
            "duplicate target profile"
        );
        assert!(symbols.insert(&target.symbol), "duplicate target symbol");
        for name in [
            &target.symbol,
            &target.layout,
            &target.backend,
            &target.native_environment,
            &target.procedure_abi,
            &target.scoop_abi,
            &target.exception_handling,
            &target.tuning,
            &target.runtime,
        ] {
            identifier(name);
        }
        let Target {
            symbol,
            canonical,
            profile,
            version,
            layout,
            backend,
            procedure_abi,
            scoop_abi,
            exception_handling,
            ..
        } = target;
        writeln!(
            output,
            "pub const {symbol}_ID: TargetProfileId = target_id({profile:?}, {version});"
        )
        .unwrap();
        writeln!(output, "pub const {symbol}: LirTargetProfile = LirTargetProfile::new(&{symbol}_ID, &crate::layout::describe({canonical:?}, crate::layout::LayoutKind::{layout}, scoop_lir::ProcedureAbi::{procedure_abi}, scoop_lir::ScoopAbiClassifier::{scoop_abi}, scoop_lir::TargetExceptionHandling::{exception_handling}));").unwrap();
        writeln!(output, "pub const {symbol}_LLVM_22_1: ValidatedLirTargetSelection = ValidatedLirTargetSelection::new({symbol}, crate::{backend});").unwrap();
        writeln!(
            source_key,
            "{canonical}\t{profile}\t{version}\t{}\t{}\t{}",
            target.os, target.arch, target.env
        )
        .unwrap();
    }
    writeln!(
        output,
        "pub const SOURCE_CATALOG_KEY: &str = {source_key:?};"
    )
    .unwrap();
    writeln!(
        output,
        "pub const TARGETS: [TargetDefinition; {}] = [",
        catalog.targets.len()
    )
    .unwrap();
    for target in &catalog.targets {
        let Target {
            symbol,
            aliases,
            versioned_aliases,
            os,
            arch,
            env,
            llvm_triple,
            native_environment,
            c_bridge_profile,
            native_rust_target,
            tuning,
            runtime,
            ..
        } = target;
        writeln!(output, "TargetDefinition {{ selection: {symbol}_LLVM_22_1, aliases: &{aliases:?}, versioned_aliases: &{versioned_aliases:?}, os: {os:?}, arch: {arch:?}, env: {env:?}, llvm_triple: {llvm_triple:?}, native_environment: NativeEnvironment::{native_environment}, c_bridge_profile: {c_bridge_profile:?}, native_rust_target: {native_rust_target:?}, tuning: CodegenTuning::{tuning}, runtime: RuntimeProfile::{runtime} }},").unwrap();
    }
    output.push_str("];\npub const fn definition(id: TargetProfileId) -> Option<TargetDefinition> { match id {\n");
    for (index, target) in catalog.targets.iter().enumerate() {
        writeln!(output, "{}_ID => Some(TARGETS[{index}]),", target.symbol).unwrap();
    }
    output.push_str("_ => None, } }\n");
    writeln!(output, "pub const HOSTS: &[HostDefinition] = &[").unwrap();
    let mut hosts = BTreeSet::new();
    for host in &catalog.hosts {
        assert!(
            hosts.insert((&host.os, &host.arch, &host.environment)),
            "duplicate host"
        );
        for target in std::iter::once(&host.default_target).chain(&host.distribution_targets) {
            assert!(
                catalog
                    .targets
                    .iter()
                    .any(|entry| &entry.canonical == target),
                "unknown host target {target}"
            );
        }
        let Host {
            os,
            arch,
            environment,
            system,
            machines,
            rust_target,
            default_target,
            distribution_targets,
        } = host;
        writeln!(output, "HostDefinition {{ os: {os:?}, arch: {arch:?}, environment: {environment:?}, system: {system:?}, machines: &{machines:?}, rust_target: {rust_target:?}, default_target: {default_target:?}, distribution_targets: &{distribution_targets:?} }},").unwrap();
    }
    output.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("catalog.rs"),
        output,
    )
    .unwrap();
}
