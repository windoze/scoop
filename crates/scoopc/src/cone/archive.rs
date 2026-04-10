//! `.cone` 归档（v0）写入与检查工具（TODO T1104）。
//!
//! 当前阶段目标：
//! - 用一个现成的 archive 格式（tar）实现最小可用的 `.cone` 包；
//! - 仅支持“写包”（`scoop package`）；读取与下游消费留给后续任务（TODO T1105+）。
//!
//! v0 归档内容（顶层文件）：
//! - `Cone.toml`：原始 manifest（文本）；
//! - `api.scoopir`：public API 的 ScoopIR（JSON，schema v0）；
//! - `ANNOTATION_CLASSES.json`：可跨 cone 导出的注解类元信息（JSON，schema v0；TODO T1016b）；
//! - `SYMBOL_VISIBILITY.json`：非 public 符号的“存在性 + 可见性层级”（JSON，schema v0；用于下游 not_visible 诊断）；
//! - `PRE_SPECIALIZE.json`：预编译函数实例索引（JSON，schema v0；TODO T1108）；
//! - `SOURCES_SHA256`：源文件内容哈希（逐文件 sha256，按相对路径排序）。

use std::io::{Cursor, Read as _};
use std::path::Path;

use miette::{Context as _, IntoDiagnostic as _, Result, miette};
use sha2::{Digest as _, Sha256};

use crate::cone::package::ConeSourcePackage;
use crate::session::Session;

/// `.cone` 内的 public API 文件名（spec §13.2）。
pub const CONE_API_SCOOPIR_FILE_NAME: &str = "api.scoopir";

/// `.cone` 内的 sources hash 文件名（v0 约定）。
pub const CONE_SOURCES_SHA256_FILE_NAME: &str = "SOURCES_SHA256";

/// 将一个 cone 源码包（目录）打包为 `.cone`（v0）。
pub fn write_cone_archive_v0(
    session: &Session,
    pkg: &ConeSourcePackage,
    output: &Path,
) -> Result<()> {
    if output.exists() && output.is_dir() {
        return Err(miette!("输出路径是目录：{}", output.display()));
    }

    let cone_toml = std::fs::read(&pkg.manifest_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("读取 Cone.toml 失败：{}", pkg.manifest_path.display()))?;

    let sources = load_pkg_sources(pkg)?;

    let api = super::scoopir::export_public_api_for_cone_sources(session, &sources)
        .wrap_err("导出 api.scoopir 失败")?;
    let api_json = serde_json::to_vec_pretty(&api)
        .into_diagnostic()
        .wrap_err("序列化 api.scoopir（JSON）失败")?;
    let api_json = ensure_trailing_newline(api_json);

    let annotation_classes =
        super::annotations::collect_cone_preserved_annotation_classes_for_cone_sources(
            session, &sources,
        )
        .wrap_err("导出 ANNOTATION_CLASSES.json 失败")?;
    let annotation_classes_json = serde_json::to_vec_pretty(&annotation_classes)
        .into_diagnostic()
        .wrap_err("序列化 ANNOTATION_CLASSES.json（JSON）失败")?;
    let annotation_classes_json = ensure_trailing_newline(annotation_classes_json);

    let symbol_visibility =
        super::visibility::collect_non_public_symbols_for_cone_sources(session, &sources)
            .wrap_err("导出 SYMBOL_VISIBILITY.json 失败")?;
    let symbol_visibility_json = serde_json::to_vec_pretty(&symbol_visibility)
        .into_diagnostic()
        .wrap_err("序列化 SYMBOL_VISIBILITY.json（JSON）失败")?;
    let symbol_visibility_json = ensure_trailing_newline(symbol_visibility_json);

    let sources_sha256 = build_sources_sha256_text(pkg)?.into_bytes();

    // T1108：可选 pre-specialize 元数据（默认不写入，以保持 v0 最小包的体积与兼容性）。
    let pre_specialize_json = super::pre_specialize::build_pre_specialize_file_for_cone_sources(
        session,
        &sources,
        &pkg.manifest,
    )?
    .map(|file| {
        let json = serde_json::to_vec_pretty(&file)
            .into_diagnostic()
            .wrap_err("序列化 PRE_SPECIALIZE.json（JSON）失败")?;
        Ok::<Vec<u8>, miette::Report>(ensure_trailing_newline(json))
    })
    .transpose()?;

    let mut owned_entries: Vec<(&'static str, Vec<u8>)> = vec![
        (super::CONE_TOML_FILE_NAME, cone_toml),
        (CONE_API_SCOOPIR_FILE_NAME, api_json),
        (
            super::annotations::CONE_ANNOTATION_CLASSES_FILE_NAME,
            annotation_classes_json,
        ),
        (
            super::visibility::CONE_SYMBOL_VISIBILITY_FILE_NAME,
            symbol_visibility_json,
        ),
    ];
    if let Some(bytes) = pre_specialize_json {
        owned_entries.push((super::pre_specialize::CONE_PRE_SPECIALIZE_FILE_NAME, bytes));
    }
    owned_entries.push((CONE_SOURCES_SHA256_FILE_NAME, sources_sha256));

    let borrowed_entries: Vec<(&str, &[u8])> = owned_entries
        .iter()
        .map(|(name, bytes)| (*name, bytes.as_slice()))
        .collect();
    write_tar_archive(output, &borrowed_entries)?;

    Ok(())
}

/// 读取 `.cone` 归档的顶层条目名（用于 `scoop package` 输出与单测）。
pub fn list_cone_archive_entries(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("打开归档失败：{}", path.display()))?;

    let mut archive = tar::Archive::new(file);
    let mut out = Vec::new();
    for entry in archive.entries().into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path().into_diagnostic()?;
        out.push(path.to_string_lossy().to_string());
    }
    out.sort();
    Ok(out)
}

/// 从 `.cone` 归档中读取指定条目的全部内容（用于单测）。
pub fn read_cone_archive_entry(path: &Path, entry_name: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("打开归档失败：{}", path.display()))?;

    let mut archive = tar::Archive::new(file);
    for entry in archive.entries().into_diagnostic()? {
        let mut entry = entry.into_diagnostic()?;
        let name = entry
            .path()
            .into_diagnostic()?
            .to_string_lossy()
            .to_string();
        if name == entry_name {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).into_diagnostic()?;
            return Ok(buf);
        }
    }

    Err(miette!(
        "归档中未找到条目：{entry_name}（归档：{}）",
        path.display()
    ))
}

/// 从 `.cone` 归档中尝试读取指定条目的全部内容（用于可选元数据的向前兼容）。
///
/// - 找到条目：`Ok(Some(bytes))`
/// - 未找到条目：`Ok(None)`
pub fn try_read_cone_archive_entry(path: &Path, entry_name: &str) -> Result<Option<Vec<u8>>> {
    let file = std::fs::File::open(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("打开归档失败：{}", path.display()))?;

    let mut archive = tar::Archive::new(file);
    for entry in archive.entries().into_diagnostic()? {
        let mut entry = entry.into_diagnostic()?;
        let name = entry
            .path()
            .into_diagnostic()?
            .to_string_lossy()
            .to_string();
        if name == entry_name {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).into_diagnostic()?;
            return Ok(Some(buf));
        }
    }

    Ok(None)
}

fn load_pkg_sources(pkg: &ConeSourcePackage) -> Result<Vec<crate::source::SourceFile>> {
    let mut out = Vec::with_capacity(pkg.sources.len());
    for path in &pkg.sources {
        out.push(crate::source::SourceFile::load(path)?);
    }
    Ok(out)
}

fn build_sources_sha256_text(pkg: &ConeSourcePackage) -> Result<String> {
    // v0：把每个源文件的内容 sha256 记录下来，作为可回归的“sources hash”。
    // 后续若引入增量编译 cache key/平台信息，可把这些元数据迁移到 CONE_META。
    let mut entries: Vec<(String, String)> = Vec::with_capacity(pkg.sources.len());

    for path in &pkg.sources {
        let rel = path.strip_prefix(&pkg.root).unwrap_or(path);
        let rel = normalize_archive_path(&rel.to_string_lossy());

        let bytes = std::fs::read(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("读取源文件失败：{}", path.display()))?;
        let digest = Sha256::digest(&bytes);
        entries.push((rel, hex_lower(&digest)));
    }

    entries.sort_by(|(a, _), (b, _)| a.cmp(b));

    let mut out = String::new();
    for (path, hash) in entries {
        // 与 sha256sum 兼容的格式：`<hex>␠␠<path>`
        out.push_str(&hash);
        out.push_str("  ");
        out.push_str(&path);
        out.push('\n');
    }
    Ok(out)
}

fn normalize_archive_path(path: &str) -> String {
    // tar/zip 里的路径分隔符统一使用 `/`，便于跨平台一致性。
    path.replace('\\', "/")
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn ensure_trailing_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}

fn write_tar_archive(path: &Path, entries: &[(&str, &[u8])]) -> Result<()> {
    let file = std::fs::File::create(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("创建归档失败：{}", path.display()))?;

    let mut builder = tar::Builder::new(file);

    for (name, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header
            .set_path(name)
            .into_diagnostic()
            .wrap_err("设置归档条目路径失败")?;
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();

        builder
            .append(&header, Cursor::new(bytes))
            .into_diagnostic()
            .wrap_err_with(|| format!("写入归档条目失败：{name}"))?;
    }

    builder
        .into_inner()
        .into_diagnostic()
        .wrap_err("写入归档失败（finish）")?;

    Ok(())
}
