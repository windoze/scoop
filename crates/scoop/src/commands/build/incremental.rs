//! Cone project input fingerprinting for the facade build driver.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoop_project_model::{ConeId, SourceConeCompilationUnit};
use sha2::{Digest as _, Sha256};

use super::super::FacadeSessionOptions;

pub(crate) const BUILD_JSON_FILE_NAME: &str = "build.json";
pub(crate) const BUILD_JSON_SCHEMA_VERSION: u32 = 4;
const CONE_INPUTS_FINGERPRINT_DOMAIN: &str = "scoop.cone.inputs.v0";
const FINAL_BUILD_FINGERPRINT_DOMAIN: &str = "scoop.build.per-cone.v0";

#[derive(Debug, Clone)]
pub(crate) struct BuildFingerprint {
    pub(crate) fingerprint: String,
    pub(crate) consumer_cone_id: ConeId,
    pub(crate) per_cone: HashMap<ConeId, ConeBuildFingerprint>,
    pub(crate) cone_toml_sha256: String,
    pub(crate) cone_sources_sha256: String,
    pub(crate) local_dependency_sources_sha256: String,
    pub(crate) native_sources_sha256: String,
    pub(crate) sysroot_sources_sha256: String,
    pub(crate) runtime_sources_sha256: String,
    pub(crate) toolchain_sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ConeBuildFingerprint {
    pub(crate) artifact_dir: PathBuf,
    pub(crate) inputs_fingerprint: Vec<u8>,
    pub(crate) cached_outputs_fingerprint: Option<Vec<u8>>,
}

pub(crate) fn compute_cone_build_fingerprint_with_session_options(
    cone_root: &Path,
    profile: &str,
    entry_package: Option<&str>,
    opt_level: scoop_project_model::OptLevel,
    session_options: &FacadeSessionOptions,
) -> Result<BuildFingerprint> {
    let pkg = scoop_project_model::load_cone_source_package(cone_root)?;
    let sysroot_root = scoop_project_model::default_sysroot_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（用于增量 fingerprint）")?;
    let graph = scoop_project_model::load_source_cone_graph_for_consumer_package(
        pkg,
        &sysroot_root,
        session_options.sysroot_overlay(),
        &[],
        session_options.extra_sysroot_dependencies(),
    )?;
    let consumer = graph.consumer();

    let cone_toml_sha256 = sha256_file(&consumer.manifest_path)?;
    let consumer_sources = consumer
        .sources
        .iter()
        .map(|source| source.path().to_path_buf())
        .collect::<Vec<_>>();
    let cone_sources_sha256 = sha256_for_files(&consumer.root, &consumer_sources)?;
    let local_dependency_sources_sha256 = sha256_for_local_dependency_nodes(&graph)?;
    let native_sources_sha256 = sha256_for_native_build_sources(&graph)?;
    let sysroot_sources_sha256 = sha256_for_selected_sysroot_cones(&graph)?;

    let runtime_root = runtime_c_dir()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 runtime/c 目录（用于增量 fingerprint）")?;
    let runtime_sources = collect_all_files_sorted(&runtime_root)?;
    let runtime_sources_sha256 = sha256_for_files(&runtime_root, &runtime_sources)?;

    let toolchain_exe = std::env::current_exe()
        .into_diagnostic()
        .wrap_err("无法定位当前 scoop 可执行文件（用于增量 fingerprint）")?;
    let toolchain_sha256 = sha256_file(&toolchain_exe)?;

    let build_dir = cone_root.join("build").join(profile).join("cones");
    let toolchain_inputs_sha256 = sha256_for_named_hex_entries([
        ("runtime", runtime_sources_sha256.as_str()),
        ("toolchain", toolchain_sha256.as_str()),
    ]);

    let mut per_cone = HashMap::new();
    let mut known_outputs: HashMap<ConeId, Vec<u8>> = HashMap::new();
    for unit in graph.compilation_units() {
        let direct_dependency_outputs_fingerprints = unit
            .dependency_cone_ids()
            .filter_map(|dep_id| known_outputs.get(&dep_id).map(|fp| (dep_id, fp.clone())))
            .collect::<Vec<_>>();
        let artifact_dir = cone_artifact_dir(&build_dir, unit);
        let inputs_fingerprint = cone_inputs_fingerprint(
            unit,
            profile,
            entry_package,
            opt_level,
            &toolchain_inputs_sha256,
            &direct_dependency_outputs_fingerprints,
        )?;
        let cached_outputs_fingerprint =
            cached_outputs_fingerprint(&artifact_dir, &inputs_fingerprint)?;
        if let Some(outputs) = cached_outputs_fingerprint.clone() {
            known_outputs.insert(unit.id(), outputs);
        } else {
            known_outputs.insert(unit.id(), inputs_fingerprint.clone());
        }
        per_cone.insert(
            unit.id(),
            ConeBuildFingerprint {
                artifact_dir,
                inputs_fingerprint,
                cached_outputs_fingerprint,
            },
        );
    }

    let consumer_cone_id = graph.consumer_id();
    let consumer_fp = per_cone.get(&consumer_cone_id).ok_or_else(|| {
        miette::miette!(
            "内部错误：per-cone fingerprint 缺少 consumer cone {}",
            consumer_cone_id.as_u32()
        )
    })?;
    let mut final_hasher = Sha256::new();
    final_hasher.update(FINAL_BUILD_FINGERPRINT_DOMAIN.as_bytes());
    final_hasher.update(b"\nentry-package=");
    final_hasher.update(entry_package.unwrap_or("").as_bytes());
    final_hasher.update(b"\nconsumer-inputs=");
    final_hasher.update(&consumer_fp.inputs_fingerprint);
    final_hasher.update(b"\n");

    Ok(BuildFingerprint {
        fingerprint: hex_lower(&final_hasher.finalize()),
        consumer_cone_id,
        per_cone,
        cone_toml_sha256,
        cone_sources_sha256,
        local_dependency_sources_sha256,
        native_sources_sha256,
        sysroot_sources_sha256,
        runtime_sources_sha256,
        toolchain_sha256,
    })
}

pub(crate) fn read_cached_fingerprint(build_json: &Path) -> Result<Option<String>> {
    if !build_json.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(build_json)
        .into_diagnostic()
        .wrap_err_with(|| format!("读取 build.json 失败：{}", build_json.display()))?;
    let json: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let schema = json.get("schema").and_then(|v| v.as_u64()).unwrap_or(0);
    if schema != u64::from(BUILD_JSON_SCHEMA_VERSION) {
        return Ok(None);
    }
    Ok(json
        .get("fingerprint")
        .and_then(|v| v.as_str())
        .map(str::to_string))
}

pub(crate) fn write_build_json(
    build_json: &Path,
    profile: &str,
    entry_package: Option<&str>,
    opt_level: scoop_project_model::OptLevel,
    fp: &BuildFingerprint,
) -> Result<()> {
    let mut per_cone = fp
        .per_cone
        .iter()
        .map(|(cone_id, cone_fp)| {
            serde_json::json!({
                "cone_id": cone_id.as_u32(),
                "artifact_dir": cone_fp.artifact_dir,
                "inputs_fingerprint": hex_lower(&cone_fp.inputs_fingerprint),
                "cached_outputs_fingerprint": cone_fp
                    .cached_outputs_fingerprint
                    .as_ref()
                    .map(|bytes| hex_lower(bytes)),
            })
        })
        .collect::<Vec<_>>();
    per_cone.sort_by_key(|entry| {
        entry
            .get("cone_id")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    });
    let value = serde_json::json!({
        "schema": BUILD_JSON_SCHEMA_VERSION,
        "profile": profile,
        "entry_package": entry_package,
        "opt_level": opt_level.as_str(),
        "fingerprint": fp.fingerprint,
        "consumer_cone_id": fp.consumer_cone_id.as_u32(),
        "per_cone": per_cone,
        "inputs": {
            "cone_toml_sha256": fp.cone_toml_sha256,
            "cone_sources_sha256": fp.cone_sources_sha256,
            "local_dependency_sources_sha256": fp.local_dependency_sources_sha256,
            "native_sources_sha256": fp.native_sources_sha256,
            "sysroot_sources_sha256": fp.sysroot_sources_sha256,
            "runtime_sources_sha256": fp.runtime_sources_sha256,
            "toolchain_sha256": fp.toolchain_sha256,
        }
    });
    let mut bytes = serde_json::to_vec_pretty(&value)
        .into_diagnostic()
        .wrap_err("序列化 build.json 失败")?;
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    std::fs::write(build_json, bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("写入 build.json 失败：{}", build_json.display()))?;
    Ok(())
}

fn cone_artifact_dir(build_cones_dir: &Path, unit: SourceConeCompilationUnit<'_>) -> PathBuf {
    let key = unit.source_cone_info().stable_key;
    build_cones_dir.join(format!("{}@{}", key.name(), key.version()))
}

fn cone_inputs_fingerprint(
    unit: SourceConeCompilationUnit<'_>,
    profile: &str,
    entry_package: Option<&str>,
    opt_level: scoop_project_model::OptLevel,
    toolchain_inputs_sha256: &str,
    direct_dependency_outputs_fingerprints: &[(ConeId, Vec<u8>)],
) -> Result<Vec<u8>> {
    let mut hasher = Sha256::new();
    hasher.update(CONE_INPUTS_FINGERPRINT_DOMAIN.as_bytes());
    hasher.update(b"\ncone=");
    let key = unit.source_cone_info().stable_key;
    hasher.update(key.name().as_bytes());
    hasher.update(b"@");
    hasher.update(key.version().as_bytes());
    hasher.update(b"\nprofile=");
    hasher.update(profile.as_bytes());
    hasher.update(b"\nopt-level=");
    hasher.update(opt_level.as_str().as_bytes());
    hasher.update(b"\ntoolchain=");
    hasher.update(toolchain_inputs_sha256.as_bytes());
    if unit.is_consumer() {
        hasher.update(b"\nentry-package=");
        hasher.update(entry_package.unwrap_or("").as_bytes());
    }
    hasher.update(b"\nmanifest=");
    hasher.update(sha256_file(&unit.node().manifest_path)?.as_bytes());
    hasher.update(b"\nsources=");
    let sources = unit
        .sources()
        .iter()
        .map(|source| source.path().to_path_buf())
        .collect::<Vec<_>>();
    hasher.update(sha256_for_files(unit.root(), &sources)?.as_bytes());
    hasher.update(b"\nnative=");
    hasher.update(sha256_for_unit_native_build_sources(unit)?.as_bytes());
    for (dep_id, outputs) in direct_dependency_outputs_fingerprints {
        hasher.update(b"\ndep-output=");
        hasher.update(dep_id.as_u32().to_le_bytes());
        hasher.update(b":");
        hasher.update(outputs);
    }
    Ok(hasher.finalize().to_vec())
}

fn cached_outputs_fingerprint(
    artifact_dir: &Path,
    inputs_fingerprint: &[u8],
) -> Result<Option<Vec<u8>>> {
    if !artifact_dir.is_dir() {
        return Ok(None);
    }
    let Ok((_manifest, cached_inputs)) =
        scoop_project_model::read_manifest_and_inputs_fingerprint(artifact_dir)
    else {
        return Ok(None);
    };
    if cached_inputs != inputs_fingerprint {
        return Ok(None);
    }
    let outputs_path =
        artifact_dir.join(scoop_project_model::CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME);
    match std::fs::read(&outputs_path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(miette::miette!(
            "读取 outputs.fingerprint 失败：{}: {err}",
            outputs_path.display()
        )),
    }
}

fn runtime_c_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime/c")
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("读取文件失败：{}", path.display()))?;
    Ok(hex_lower(&Sha256::digest(&bytes)))
}

fn sha256_for_files(root: &Path, files: &[PathBuf]) -> Result<String> {
    let mut entries = Vec::with_capacity(files.len());
    for path in files {
        let rel = normalize_rel_path_forward_slashes(root, path);
        entries.push((rel, path.to_path_buf()));
    }
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    let mut hasher = Sha256::new();
    for (rel, abs) in entries {
        let file_hash = sha256_file(&abs)?;
        hasher.update(rel.as_bytes());
        hasher.update(b"\n");
        hasher.update(file_hash.as_bytes());
        hasher.update(b"\n");
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn sha256_for_local_dependency_nodes(
    graph: &scoop_project_model::SourceConeGraph,
) -> Result<String> {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for node in graph.nodes() {
        if node.role != scoop_project_model::SourceConeRole::LocalDependency {
            continue;
        }
        entries.push((
            format!("{}/Cone.toml", node.manifest.cone.name),
            node.manifest_path.clone(),
        ));
        for source in &node.sources {
            let rel = normalize_rel_path_forward_slashes(&node.root, source.path());
            entries.push((
                format!("{}/{}", node.manifest.cone.name, rel),
                source.path().to_path_buf(),
            ));
        }
    }
    sha256_for_named_files(entries)
}

fn sha256_for_native_build_sources(graph: &scoop_project_model::SourceConeGraph) -> Result<String> {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for node in graph.nodes() {
        let native_build = &node.native_build;
        if native_build.c_sources.is_empty() && native_build.cxx_sources.is_empty() {
            continue;
        }
        entries.push((
            format!("{}/Cone.toml", node.manifest.cone.name),
            node.manifest_path.clone(),
        ));
        for rel in &native_build.c_sources {
            let path = node.root.join(rel);
            let rel = normalize_rel_path_forward_slashes(&node.root, &path);
            entries.push((
                format!("{}/native-build/c/{rel}", node.manifest.cone.name),
                path,
            ));
        }
        for rel in &native_build.cxx_sources {
            let path = node.root.join(rel);
            let rel = normalize_rel_path_forward_slashes(&node.root, &path);
            entries.push((
                format!("{}/native-build/cxx/{rel}", node.manifest.cone.name),
                path,
            ));
        }
    }
    sha256_for_named_files(entries)
}

fn sha256_for_selected_sysroot_cones(
    graph: &scoop_project_model::SourceConeGraph,
) -> Result<String> {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for node in graph.nodes() {
        if node.role != scoop_project_model::SourceConeRole::SysrootAuto {
            continue;
        }
        entries.push((
            format!("{}/Cone.toml", node.manifest.cone.name),
            node.manifest_path.clone(),
        ));
        for source in &node.sources {
            let rel = normalize_rel_path_forward_slashes(&node.root, source.path());
            entries.push((
                format!("{}/{rel}", node.manifest.cone.name),
                source.path().to_path_buf(),
            ));
        }
    }
    sha256_for_named_files(entries)
}

fn sha256_for_unit_native_build_sources(unit: SourceConeCompilationUnit<'_>) -> Result<String> {
    let native_build = &unit.node().native_build;
    let mut entries = Vec::new();
    for rel in &native_build.c_sources {
        let path = unit.root().join(rel);
        let rel = normalize_rel_path_forward_slashes(unit.root(), &path);
        entries.push((format!("native-build/c/{rel}"), path));
    }
    for rel in &native_build.cxx_sources {
        let path = unit.root().join(rel);
        let rel = normalize_rel_path_forward_slashes(unit.root(), &path);
        entries.push((format!("native-build/cxx/{rel}"), path));
    }
    sha256_for_named_files(entries)
}

fn sha256_for_named_files(mut entries: Vec<(String, PathBuf)>) -> Result<String> {
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    let mut hasher = Sha256::new();
    for (rel, abs) in entries {
        let file_hash = sha256_file(&abs)?;
        hasher.update(rel.as_bytes());
        hasher.update(b"\n");
        hasher.update(file_hash.as_bytes());
        hasher.update(b"\n");
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn sha256_for_named_hex_entries<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let mut entries = entries.into_iter().collect::<Vec<_>>();
    entries.sort_by_key(|(lhs, _)| *lhs);
    let mut hasher = Sha256::new();
    for (name, digest) in entries {
        hasher.update(name.as_bytes());
        hasher.update(b"=");
        hasher.update(digest.as_bytes());
        hasher.update(b"\n");
    }
    hex_lower(&hasher.finalize())
}

fn normalize_rel_path_forward_slashes(root: &Path, abs: &Path) -> String {
    let rel = abs.strip_prefix(root).unwrap_or(abs);
    rel.to_string_lossy().replace('\\', "/")
}

fn collect_all_files_sorted(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_all_files(root, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_all_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;
        if ty.is_dir() {
            collect_all_files(&path, out)?;
        } else if ty.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn build_json_roundtrips_cached_fingerprint() {
        let dir = tempdir().unwrap();
        let build_json = dir.path().join(BUILD_JSON_FILE_NAME);
        let fp = BuildFingerprint {
            fingerprint: "abc".to_string(),
            consumer_cone_id: ConeId::new(1),
            per_cone: HashMap::new(),
            cone_toml_sha256: String::new(),
            cone_sources_sha256: String::new(),
            local_dependency_sources_sha256: String::new(),
            native_sources_sha256: String::new(),
            sysroot_sources_sha256: String::new(),
            runtime_sources_sha256: String::new(),
            toolchain_sha256: String::new(),
        };

        write_build_json(
            &build_json,
            "debug",
            None,
            scoop_project_model::OptLevel::O0,
            &fp,
        )
        .unwrap();

        assert_eq!(
            read_cached_fingerprint(&build_json).unwrap().as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn stale_artifact_inputs_do_not_count_as_cache_hit() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path()
                .join(scoop_project_model::CONE_ARTIFACT_MANIFEST_FILE_NAME),
            serde_json::to_vec(&scoop_project_model::ConeArtifactManifest::current(
                &scoop_project_model::StableConeKey::new("demo", "0.0.0"),
                scoop_project_model::ConeKind::Lib,
                Vec::new(),
            ))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path()
                .join(scoop_project_model::CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME),
            b"old",
        )
        .unwrap();
        std::fs::write(
            dir.path()
                .join(scoop_project_model::CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME),
            b"outputs",
        )
        .unwrap();

        assert!(
            cached_outputs_fingerprint(dir.path(), b"new")
                .unwrap()
                .is_none()
        );
    }
}
