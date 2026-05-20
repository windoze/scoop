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

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};
use sha2::{Digest as _, Sha256};

use scoopc::opt::OptLevel;

pub(crate) const BUILD_JSON_FILE_NAME: &str = "build.json";
pub(crate) const BUILD_JSON_SCHEMA_VERSION: u32 = 3;

/// 本次 build 的输入 fingerprint。
///
/// - `fingerprint`：最终用于 cache 命中的总 fingerprint。
/// - 其它字段：用于调试/排查“为什么没命中缓存”的原因（不会参与命中判断）。
#[derive(Debug, Clone)]
pub(crate) struct BuildFingerprint {
    pub(crate) fingerprint: String,
    pub(crate) cone_toml_sha256: String,
    pub(crate) cone_sources_sha256: String,
    pub(crate) local_dependency_sources_sha256: String,
    pub(crate) native_sources_sha256: String,
    pub(crate) sysroot_sources_sha256: String,
    pub(crate) runtime_sources_sha256: String,
    pub(crate) toolchain_sha256: String,
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
    let graph = scoopc::cone::SourceConeGraph::load_for_consumer_package(
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

    // 总 fingerprint：把所有“会影响产物”的输入摘要与关键 flags 组合在一起。
    let mut hasher = Sha256::new();
    hasher.update(b"scoop.build.fingerprint.v3\n");
    hasher.update(b"profile=");
    hasher.update(profile.as_bytes());
    hasher.update(b"\n");
    hasher.update(b"entry-package=");
    hasher.update(entry_package.unwrap_or("").as_bytes());
    hasher.update(b"\n");
    hasher.update(b"opt-level=");
    hasher.update(opt_level.as_str().as_bytes());
    hasher.update(b"\n");
    hasher.update(b"cone_toml=");
    hasher.update(cone_toml_sha256.as_bytes());
    hasher.update(b"\n");
    hasher.update(b"cone_sources=");
    hasher.update(cone_sources_sha256.as_bytes());
    hasher.update(b"\n");
    hasher.update(b"local_dependencies=");
    hasher.update(local_dependency_sources_sha256.as_bytes());
    hasher.update(b"\n");
    hasher.update(b"native_sources=");
    hasher.update(native_sources_sha256.as_bytes());
    hasher.update(b"\n");
    hasher.update(b"sysroot=");
    hasher.update(sysroot_sources_sha256.as_bytes());
    hasher.update(b"\n");
    hasher.update(b"runtime=");
    hasher.update(runtime_sources_sha256.as_bytes());
    hasher.update(b"\n");
    hasher.update(b"toolchain=");
    hasher.update(toolchain_sha256.as_bytes());
    hasher.update(b"\n");

    let fingerprint = hex_lower(&hasher.finalize());

    Ok(BuildFingerprint {
        fingerprint,
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
    let value = serde_json::json!({
        "schema": BUILD_JSON_SCHEMA_VERSION,
        "profile": profile,
        "entry_package": entry_package,
        "opt_level": opt_level.as_str(),
        "fingerprint": fp.fingerprint,
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
}
