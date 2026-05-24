//! Cone 项目的粗粒度增量构建（T1124）。
//!
//! 目标（v1）：
//! - 为 cone 项目计算一个“输入 fingerprint”（粗粒度 cache key）；
//! - 将 fingerprint 写入 `build/<profile>/build.json`；
//! - 当 fingerprint 未变化且可执行文件已存在时，允许跳过 build（cache hit）。
//!
//! 说明：
//! - 这里不做依赖图，也不做“只重建受影响文件”（那是 v2+ 的范围）；
//! - fingerprint 的设计偏向“正确优先”：宁可多 rebuild，也不要错误复用旧产物；
//! - 为避免跨机器路径差异带来的抖动，文件列表按“相对路径（forward slashes）”排序后参与哈希。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};
use sha2::{Digest as _, Sha256};

use scoopc::cone::{ConeId, SourceConeCompilationUnit, SourceConeRole};
use scoopc::opt::OptLevel;

pub(crate) const BUILD_JSON_FILE_NAME: &str = "build.json";
pub(crate) const BUILD_JSON_SCHEMA_VERSION: u32 = 4;
const CONE_INPUTS_FINGERPRINT_DOMAIN: &str = "scoop.cone.inputs.v0";
const FINAL_BUILD_FINGERPRINT_DOMAIN: &str = "scoop.build.per-cone.v0";

/// 本次 build 的输入 fingerprint。
///
/// - `fingerprint`：最终用于 cache 命中的总 fingerprint。
/// - 其它字段：用于调试/排查“为什么没命中缓存”的原因（不会参与命中判断）。
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
    pub(crate) direct_dependency_outputs_fingerprints: Vec<(ConeId, Vec<u8>)>,
}

/// 计算 cone 项目 build 的粗粒度 fingerprint。
///
/// 最低要求（TODO T1124）：至少包含
/// - `Cone.toml`
/// - `src/**/*.scoop`
/// - 本地 source path dependency 的 `Cone.toml` 与 sources
/// - 关键 build flags（profile/entry-package）
/// - 工具链版本
///
/// 这里额外纳入：
/// - sysroot（`sysroot/*.scoop`）
/// - loaded source cones 声明的 native C/C++ sources
/// - C runtime（`runtime/c/**`）
///
/// 原因：这些文件会影响最终产物，但通常不会改变 `scoop` 可执行文件本体。
pub(crate) fn compute_cone_build_fingerprint(
    cone_root: &Path,
    profile: &str,
    entry_package: Option<&str>,
    opt_level: OptLevel,
) -> Result<BuildFingerprint> {
    let pkg = scoopc::cone::load_cone_source_package(cone_root)?;
    let sysroot_root = scoopc::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（用于增量 fingerprint）")?;
    let graph = scoopc::cone::load_source_cone_graph_for_consumer_package(
        pkg,
        &sysroot_root,
        None,
        &[],
        &[],
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

    let sysroot_sources = collect_scoop_files_sorted(&sysroot_root)?;
    let sysroot_sources_sha256 = sha256_for_files(&sysroot_root, &sysroot_sources)?;

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
        ("sysroot", sysroot_sources_sha256.as_str()),
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
                direct_dependency_outputs_fingerprints,
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
    let fingerprint = hex_lower(&final_hasher.finalize());

    Ok(BuildFingerprint {
        fingerprint,
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

/// 从 `build.json` 读取缓存 fingerprint；若文件不存在/格式不兼容则返回 `Ok(None)`。
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

    let fingerprint = json.get("fingerprint").and_then(|v| v.as_str());
    Ok(fingerprint.map(str::to_string))
}

/// 将 fingerprint 写入 `build.json`（pretty JSON + 末尾换行）。
pub(crate) fn write_build_json(
    build_json: &Path,
    profile: &str,
    entry_package: Option<&str>,
    opt_level: OptLevel,
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

pub(crate) fn frontend_artifact_cache_for_build(
    input: &scoopc::frontend::ProjectInput,
    _profile: &str,
    fp: &BuildFingerprint,
) -> scoopc::frontend::FrontendArtifactCache {
    let mut cache = scoopc::frontend::FrontendArtifactCache::new();
    for unit in input.compilation_units() {
        if unit.is_consumer() || unit.role() != SourceConeRole::LocalDependency {
            continue;
        }
        let Some(cone_fp) = fp.per_cone.get(&unit.id()) else {
            continue;
        };
        cache.insert(
            unit.id(),
            scoopc::frontend::FrontendArtifactCacheEntry::new(
                cone_fp.artifact_dir.clone(),
                cone_fp.inputs_fingerprint.clone(),
            )
            .with_dependency_outputs_fingerprints(
                cone_fp.direct_dependency_outputs_fingerprints.clone(),
            ),
        );
    }
    cache
}

fn cone_artifact_dir(build_cones_dir: &Path, unit: SourceConeCompilationUnit<'_>) -> PathBuf {
    let key = unit.source_cone_info().stable_key;
    build_cones_dir.join(format!("{}@{}", key.name(), key.version()))
}

fn cone_inputs_fingerprint(
    unit: SourceConeCompilationUnit<'_>,
    profile: &str,
    entry_package: Option<&str>,
    opt_level: OptLevel,
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
    match scoopc::cone::ConeArtifact::read_with_inputs_fingerprint(artifact_dir, inputs_fingerprint)
    {
        Ok(artifact) => Ok(Some(artifact.outputs_fingerprint)),
        Err(scoopc::cone::ConeArtifactError::InputsFingerprintMismatch { .. }) => Ok(None),
        Err(scoopc::cone::ConeArtifactError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(None)
        }
        Err(err) => Err(miette::miette!("{err}")),
    }
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("读取文件失败：{}", path.display()))?;
    Ok(hex_lower(&Sha256::digest(&bytes)))
}

/// 计算一组文件的摘要（按“相对路径 + 内容 sha256”组合）。
fn sha256_for_files(root: &Path, files: &[PathBuf]) -> Result<String> {
    let mut entries: Vec<(String, PathBuf)> = Vec::with_capacity(files.len());
    for path in files {
        let rel = normalize_rel_path_forward_slashes(root, path)?;
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

fn sha256_for_local_dependency_nodes(graph: &scoopc::cone::SourceConeGraph) -> Result<String> {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for node in graph.nodes() {
        if node.role != scoopc::cone::SourceConeRole::LocalDependency {
            continue;
        }

        entries.push((
            format!("{}/Cone.toml", node.manifest.cone.name),
            node.manifest_path.clone(),
        ));
        for source in &node.sources {
            let rel = normalize_rel_path_forward_slashes(&node.root, source.path())?;
            entries.push((
                format!("{}/{}", node.manifest.cone.name, rel),
                source.path().to_path_buf(),
            ));
        }
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

fn sha256_for_native_build_sources(graph: &scoopc::cone::SourceConeGraph) -> Result<String> {
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
            let rel = normalize_rel_path_forward_slashes(&node.root, &path)?;
            entries.push((
                format!("{}/native-build/c/{rel}", node.manifest.cone.name),
                path,
            ));
        }
        for rel in &native_build.cxx_sources {
            let path = node.root.join(rel);
            let rel = normalize_rel_path_forward_slashes(&node.root, &path)?;
            entries.push((
                format!("{}/native-build/cxx/{rel}", node.manifest.cone.name),
                path,
            ));
        }
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

fn sha256_for_unit_native_build_sources(unit: SourceConeCompilationUnit<'_>) -> Result<String> {
    let native_build = &unit.node().native_build;
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for rel in &native_build.c_sources {
        let path = unit.root().join(rel);
        let rel = normalize_rel_path_forward_slashes(unit.root(), &path)?;
        entries.push((format!("native-build/c/{rel}"), path));
    }
    for rel in &native_build.cxx_sources {
        let path = unit.root().join(rel);
        let rel = normalize_rel_path_forward_slashes(unit.root(), &path)?;
        entries.push((format!("native-build/cxx/{rel}"), path));
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

fn normalize_rel_path_forward_slashes(root: &Path, abs: &Path) -> Result<String> {
    let rel = abs.strip_prefix(root).unwrap_or(abs);
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn collect_scoop_files_sorted(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_scoop_files(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;

        if ty.is_dir() {
            collect_scoop_files(&path, out)?;
            continue;
        }

        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
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
            continue;
        }

        if ty.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn runtime_c_dir() -> PathBuf {
    // 开发期路径：相对于 `crates/scoop` 的 `../../runtime/c`。
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime/c")
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
    use scoopc::cone::{ConeArtifact, ConeArtifactFingerprints, ConeArtifactStageProducts};
    use scoopc::effect_facts_product::EffectFacts;
    use scoopc::effect_lowered::LateLoweredProgram;
    use scoopc::hir_facts::HirFacts;
    use scoopc::lir_facts_product::LirFacts;
    use scoopc::mir_facts::MirFacts;
    use tempfile::tempdir;

    #[test]
    fn fingerprint_roundtrips_via_build_json_and_changes_on_source_update() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("app");
        std::fs::create_dir_all(root.join("src")).unwrap();

        std::fs::write(
            root.join("Cone.toml"),
            r#"
[cone]
name = "fixture-incremental"
version = "0.0.0"
kind = "bin"
"#,
        )
        .unwrap();
        std::fs::write(root.join("src/main.scoop"), "fun main() {}\n").unwrap();

        let fp1 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();

        let build_dir = root.join("build").join("debug");
        std::fs::create_dir_all(&build_dir).unwrap();
        let build_json = build_dir.join(BUILD_JSON_FILE_NAME);

        write_build_json(&build_json, "debug", None, OptLevel::O0, &fp1).unwrap();
        let cached = read_cached_fingerprint(&build_json).unwrap().unwrap();
        assert_eq!(cached, fp1.fingerprint);

        std::fs::write(root.join("src/main.scoop"), "fun main() { }\n").unwrap();
        let fp2 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        assert_ne!(fp1.fingerprint, fp2.fingerprint);
    }

    #[test]
    fn fingerprint_changes_on_local_dependency_source_update() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("app");
        let dep = root.join("deps").join("lib");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(dep.join("src")).unwrap();

        std::fs::write(
            root.join("Cone.toml"),
            r#"
[cone]
name = "fixture-incremental-app"
version = "0.0.0"
kind = "bin"

[dependencies]
"fixture-incremental-lib" = { path = "deps/lib" }
"#,
        )
        .unwrap();
        std::fs::write(root.join("src/main.scoop"), "fun main() {}\n").unwrap();
        std::fs::write(
            dep.join("Cone.toml"),
            r#"
[cone]
name = "fixture-incremental-lib"
version = "0.0.0"
kind = "lib"
"#,
        )
        .unwrap();
        std::fs::write(
            dep.join("src/api.scoop"),
            "package dep\nfun value(): Int { return 1 }\n",
        )
        .unwrap();

        let fp1 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        std::fs::write(
            dep.join("src/api.scoop"),
            "package dep\nfun value(): Int { return 2 }\n",
        )
        .unwrap();
        let fp2 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();

        assert_ne!(fp1.fingerprint, fp2.fingerprint);
        assert_ne!(
            fp1.local_dependency_sources_sha256,
            fp2.local_dependency_sources_sha256
        );
    }

    #[test]
    fn fingerprint_changes_on_local_dependency_native_source_update() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("app");
        let dep = root.join("deps").join("lib");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(dep.join("src")).unwrap();
        std::fs::create_dir_all(dep.join("native")).unwrap();

        std::fs::write(
            root.join("Cone.toml"),
            r#"
[cone]
name = "fixture-incremental-native-app"
version = "0.0.0"
kind = "bin"

[dependencies]
"fixture-incremental-native-lib" = { path = "deps/lib" }
"#,
        )
        .unwrap();
        std::fs::write(root.join("src/main.scoop"), "fun main() {}\n").unwrap();
        std::fs::write(
            dep.join("Cone.toml"),
            r#"
[cone]
name = "fixture-incremental-native-lib"
version = "0.0.0"
kind = "lib"

[native-build]
c-sources = ["native/add.c"]
"#,
        )
        .unwrap();
        std::fs::write(
            dep.join("src/api.scoop"),
            "package dep\nfun value(): Int { return 1 }\n",
        )
        .unwrap();
        std::fs::write(
            dep.join("native/add.c"),
            "int dep_add(void) { return 1; }\n",
        )
        .unwrap();

        let fp1 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        std::fs::write(
            dep.join("native/add.c"),
            "int dep_add(void) { return 2; }\n",
        )
        .unwrap();
        let fp2 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();

        assert_ne!(fp1.fingerprint, fp2.fingerprint);
        assert_ne!(fp1.native_sources_sha256, fp2.native_sources_sha256);
    }

    #[test]
    fn per_cone_chain_rebuilds_only_user_cone_when_user_source_changes() {
        let dir = tempdir().unwrap();
        let root = write_app_with_local_dependency(dir.path());

        let fp1 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        let dep_id = only_dependency_cone_id(&fp1);
        write_minimal_artifact_for_cone(&fp1, dep_id);

        let fp2 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        let dep_before = fp2.per_cone.get(&dep_id).unwrap();
        assert!(dep_before.cached_outputs_fingerprint.is_some());

        std::fs::write(
            root.join("src/main.scoop"),
            "package app\nimport dep.*\nfun main() { value(); () }\n",
        )
        .unwrap();
        let fp3 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        let dep_after = fp3.per_cone.get(&dep_id).unwrap();

        assert_eq!(
            dep_before.inputs_fingerprint, dep_after.inputs_fingerprint,
            "user source edits must not invalidate dependency cone inputs"
        );
        assert_eq!(
            dep_before.cached_outputs_fingerprint, dep_after.cached_outputs_fingerprint,
            "dependency cone artifact should remain reusable"
        );
        assert_ne!(fp2.fingerprint, fp3.fingerprint);
    }

    #[test]
    fn per_cone_chain_invalidates_dependency_and_user_when_dependency_source_changes() {
        let dir = tempdir().unwrap();
        let root = write_app_with_local_dependency(dir.path());

        let fp1 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        let dep_id = only_dependency_cone_id(&fp1);
        write_minimal_artifact_for_cone(&fp1, dep_id);
        let fp2 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();

        std::fs::write(
            root.join("deps/lib/src/api.scoop"),
            "package dep\npublic fun value(): Int { return 2 }\n",
        )
        .unwrap();
        let fp3 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();

        assert_ne!(
            fp2.per_cone.get(&dep_id).unwrap().inputs_fingerprint,
            fp3.per_cone.get(&dep_id).unwrap().inputs_fingerprint
        );
        assert!(
            fp3.per_cone
                .get(&dep_id)
                .unwrap()
                .cached_outputs_fingerprint
                .is_none()
        );
        assert_ne!(fp2.fingerprint, fp3.fingerprint);
    }

    #[test]
    fn per_cone_chain_invalidates_all_cones_when_toolchain_inputs_change() {
        let dir = tempdir().unwrap();
        let root = write_app_with_local_dependency(dir.path());

        let fp1 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        let fp2 = compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O2).unwrap();

        for (cone_id, cone_fp1) in &fp1.per_cone {
            assert_ne!(
                cone_fp1.inputs_fingerprint,
                fp2.per_cone.get(cone_id).unwrap().inputs_fingerprint
            );
        }
        assert_ne!(fp1.fingerprint, fp2.fingerprint);
    }

    /// 防止回归：`build.rs` 必须在 build 完成（依赖 cone 的 artifact 已写盘）之后
    /// 重新计算 fingerprint 并写入 `build.json`。否则下一次无修改运行会因
    /// “build.json 里存的是 placeholder fingerprint，重新计算用的是真实
    /// outputs fingerprint” 而 cache miss，破坏 user cone short-circuit。
    #[test]
    fn per_cone_chain_post_build_fingerprint_matches_next_run() {
        let dir = tempdir().unwrap();
        let root = write_app_with_local_dependency(dir.path());

        // 第一次：依赖 cone 还没产物，consumer 用 placeholder=inputs_fingerprint。
        let fp_pre_build =
            compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        let dep_id = only_dependency_cone_id(&fp_pre_build);
        assert!(
            fp_pre_build
                .per_cone
                .get(&dep_id)
                .unwrap()
                .cached_outputs_fingerprint
                .is_none(),
            "no artifact on disk yet"
        );

        // 模拟 build 期间：依赖 cone 的 artifact 落盘，带上真实 outputs.fingerprint。
        write_minimal_artifact_for_cone(&fp_pre_build, dep_id);

        // build 完成立刻重新计算（即 build.rs 在 build 之后执行的那一次）。
        let fp_post_build =
            compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        let dep_real_outputs = fp_post_build
            .per_cone
            .get(&dep_id)
            .unwrap()
            .cached_outputs_fingerprint
            .clone()
            .expect("dep artifact must be on disk after write_minimal_artifact_for_cone");
        let dep_placeholder = fp_pre_build
            .per_cone
            .get(&dep_id)
            .unwrap()
            .inputs_fingerprint
            .clone();
        assert_ne!(
            dep_placeholder, dep_real_outputs,
            "placeholder (=dep inputs) and real outputs must diverge — that's why \
             build.rs has to recompute the fingerprint after the build"
        );
        assert_ne!(
            fp_pre_build.fingerprint, fp_post_build.fingerprint,
            "consumer fingerprint must change once dep outputs land on disk"
        );

        // 模拟下一次无修改的运行：磁盘状态没变，必须命中缓存。
        let fp_next_run =
            compute_cone_build_fingerprint(&root, "debug", None, OptLevel::O0).unwrap();
        assert_eq!(
            fp_post_build.fingerprint, fp_next_run.fingerprint,
            "post-build fingerprint must equal what the next run computes — \
             this is the user-cone short-circuit invariant"
        );
        let consumer_id = fp_post_build.consumer_cone_id;
        assert_eq!(
            fp_post_build
                .per_cone
                .get(&consumer_id)
                .unwrap()
                .inputs_fingerprint,
            fp_next_run
                .per_cone
                .get(&consumer_id)
                .unwrap()
                .inputs_fingerprint,
            "consumer inputs.fingerprint must round-trip stably across runs"
        );
    }

    fn write_app_with_local_dependency(temp_root: &Path) -> PathBuf {
        let root = temp_root.join("app");
        let dep = root.join("deps").join("lib");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(dep.join("src")).unwrap();
        std::fs::write(
            root.join("Cone.toml"),
            r#"
[cone]
name = "fixture-per-cone-app"
version = "0.0.0"
kind = "bin"

[dependencies]
dep = { path = "deps/lib" }
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("src/main.scoop"),
            "package app\nimport dep.*\nfun main() { value() }\n",
        )
        .unwrap();
        std::fs::write(
            dep.join("Cone.toml"),
            r#"
[cone]
name = "dep"
version = "0.0.0"
kind = "lib"
"#,
        )
        .unwrap();
        std::fs::write(
            dep.join("src/api.scoop"),
            "package dep\npublic fun value(): Int { return 1 }\n",
        )
        .unwrap();
        root
    }

    fn only_dependency_cone_id(fp: &BuildFingerprint) -> ConeId {
        fp.per_cone
            .iter()
            .find(|(_, cone_fp)| {
                cone_fp
                    .artifact_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    == Some("dep@0.0.0")
            })
            .map(|(cone_id, _)| *cone_id)
            .expect("fixture should have one dependency cone")
    }

    fn write_minimal_artifact_for_cone(fp: &BuildFingerprint, cone_id: ConeId) {
        let cone_fp = fp.per_cone.get(&cone_id).unwrap();
        let mut artifact = ConeArtifact::with_parts(
            scoopc::stable_id::StableConeKey::new("dep", "0.0.0"),
            scoopc::base::project_model::ConeKind::Lib,
            ConeArtifactStageProducts::new(
                HirFacts::new(),
                MirFacts::new(),
                EffectFacts::new(),
                LirFacts::new(OptLevel::O0),
                LateLoweredProgram::new(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            ),
            scoopc::cone::ConeArtifactFrontendImport::empty(),
            Vec::new(),
            ConeArtifactFingerprints::new(cone_fp.inputs_fingerprint.clone(), Vec::new()),
        );
        artifact
            .write_with_computed_outputs_fingerprint(&cone_fp.artifact_dir)
            .unwrap();
    }
}
