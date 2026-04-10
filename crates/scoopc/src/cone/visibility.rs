//! `.cone` 依赖的“符号可见性索引”（v0）。
//!
//! 背景：
//! - `api.scoopir` 只包含依赖 cone 的 `public` API（T1103/T1104）。
//! - 但对下游来说，引用依赖 cone 的 `internal/private` 符号应该得到稳定诊断
//!   `scoop::resolve::not_visible`（TODO T0321b），而不是“未解析”。
//!
//! 因此 v0 额外在 `.cone` 中写入一个轻量 JSON 文件：
//! - 仅记录**非 public** 的符号“存在性 + 可见性层级”（不含签名/类型信息）
//! - 下游消费时把这些符号以“不可见占位符”的形式注入 resolver `Index`
//! - 这样下游在引用它们时能得到稳定的 `not_visible` 诊断

use std::collections::HashSet;

use miette::{Context as _, IntoDiagnostic as _, Result};
use serde::{Deserialize, Serialize};

use crate::resolve::{ConeId, Index, IndexedFile, Visibility};
use crate::session::Session;
use crate::source::SourceFile;

/// `.cone` 内的符号可见性索引文件名（v0 约定）。
pub const CONE_SYMBOL_VISIBILITY_FILE_NAME: &str = "SYMBOL_VISIBILITY.json";

pub const CONE_SYMBOL_VISIBILITY_SCHEMA_NAME: &str = "scoop.cone.symbol_visibility";
pub const CONE_SYMBOL_VISIBILITY_SCHEMA_VERSION: u32 = 0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConeSymbolVisibilitySchema {
    pub name: String,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConeSymbolKind {
    Type,
    Value,
    Fun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConeSymbolVisibility {
    Public,
    Internal,
    Private,
}

impl From<Visibility> for ConeSymbolVisibility {
    fn from(v: Visibility) -> Self {
        match v {
            Visibility::Public => ConeSymbolVisibility::Public,
            Visibility::Internal => ConeSymbolVisibility::Internal,
            Visibility::Private => ConeSymbolVisibility::Private,
        }
    }
}

impl From<ConeSymbolVisibility> for Visibility {
    fn from(v: ConeSymbolVisibility) -> Self {
        match v {
            ConeSymbolVisibility::Public => Visibility::Public,
            ConeSymbolVisibility::Internal => Visibility::Internal,
            ConeSymbolVisibility::Private => Visibility::Private,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeSymbolVisibilityEntry {
    pub kind: ConeSymbolKind,
    pub fqn: String,
    pub visibility: ConeSymbolVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConeSymbolVisibilityFile {
    pub schema: ConeSymbolVisibilitySchema,
    pub symbols: Vec<ConeSymbolVisibilityEntry>,
}

impl ConeSymbolVisibilityFile {
    pub fn new_v0(mut symbols: Vec<ConeSymbolVisibilityEntry>) -> Self {
        symbols.sort_by(|a, b| {
            a.fqn
                .cmp(&b.fqn)
                .then(a.kind.cmp(&b.kind))
                .then(a.visibility.cmp(&b.visibility))
        });
        symbols.dedup_by(|a, b| a.fqn == b.fqn && a.kind == b.kind);

        Self {
            schema: ConeSymbolVisibilitySchema {
                name: CONE_SYMBOL_VISIBILITY_SCHEMA_NAME.to_string(),
                version: CONE_SYMBOL_VISIBILITY_SCHEMA_VERSION,
            },
            symbols,
        }
    }
}

/// 生成 cone 的“非 public 符号”索引（用于下游 not_visible 诊断）。
///
/// 约定（v0）：
/// - sysroot cone=0，当前 cone=1；
/// - 仅输出 `decl_file` 属于 `sources` 的符号；
/// - 仅输出 **visibility != public** 的符号；
/// - fun 以 “FQN 级别”记录（只要该 FQN 下所有 overload 都是非 public，就记录 1 条）。
pub fn collect_non_public_symbols_for_cone_sources(
    session: &Session,
    sources: &[SourceFile],
) -> Result<ConeSymbolVisibilityFile> {
    let source_paths: HashSet<_> = sources.iter().map(|s| s.path().to_path_buf()).collect();

    // 1) parse sources → AST（Index 构建需要 AST 引用）。
    let mut asts = Vec::with_capacity(sources.len());
    for source in sources {
        let mut ast = crate::parser::parse_file(source).map_err(miette::Report::from)?;
        crate::comptime::trim_package_level_comptime_ifs(source, &mut ast)
            .map_err(miette::Report::from)?;
        asts.push(ast);
    }

    // 2) build index：sysroot cone=0，当前 cone=1（与 ScoopIR 导出保持一致）。
    let mut indexed: Vec<IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(IndexedFile {
            cone: ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source, ast) in sources.iter().zip(asts.iter()) {
        indexed.push(IndexedFile {
            cone: ConeId::new(1),
            source,
            file: ast,
        });
    }

    let index = Index::build_with_cones(&indexed).map_err(miette::Report::from)?;

    // 3) 从 index 收集非 public 的顶层符号。
    let mut out: Vec<ConeSymbolVisibilityEntry> = Vec::new();
    for (fqn, ns) in &index.by_fqn {
        if let Some(sym) = ns.ty.as_ref()
            && source_paths.contains(sym.decl_file.as_path())
                && sym.visibility != Visibility::Public
            {
                out.push(ConeSymbolVisibilityEntry {
                    kind: ConeSymbolKind::Type,
                    fqn: fqn.clone(),
                    visibility: sym.visibility.into(),
                });
            }

        if let Some(sym) = ns.value.as_ref()
            && source_paths.contains(sym.decl_file.as_path())
                && sym.visibility != Visibility::Public
            {
                out.push(ConeSymbolVisibilityEntry {
                    kind: ConeSymbolKind::Value,
                    fqn: fqn.clone(),
                    visibility: sym.visibility.into(),
                });
            }

        if !ns.fun.is_empty() {
            let declared = ns
                .fun
                .iter()
                .filter(|o| source_paths.contains(o.symbol.decl_file.as_path()))
                .collect::<Vec<_>>();
            if declared.is_empty() {
                continue;
            }
            let has_public = declared
                .iter()
                .any(|o| o.symbol.visibility == Visibility::Public);
            if has_public {
                continue;
            }

            // v0：只要该 FQN 的 overload 全部非 public，就记录 1 条占位符（用于 not_visible）。
            let vis = declared[0].symbol.visibility;
            out.push(ConeSymbolVisibilityEntry {
                kind: ConeSymbolKind::Fun,
                fqn: fqn.clone(),
                visibility: vis.into(),
            });
        }
    }

    Ok(ConeSymbolVisibilityFile::new_v0(out))
}

pub fn parse_symbol_visibility_file(bytes: &[u8]) -> Result<ConeSymbolVisibilityFile> {
    let file: ConeSymbolVisibilityFile = serde_json::from_slice(bytes)
        .into_diagnostic()
        .wrap_err("解析 SYMBOL_VISIBILITY.json 失败")?;

    if file.schema.name != CONE_SYMBOL_VISIBILITY_SCHEMA_NAME {
        return Err(miette::miette!(
            "SYMBOL_VISIBILITY.json schema.name 不匹配：期望 `{CONE_SYMBOL_VISIBILITY_SCHEMA_NAME}`，但得到 `{}`",
            file.schema.name
        ));
    }
    if file.schema.version != CONE_SYMBOL_VISIBILITY_SCHEMA_VERSION {
        return Err(miette::miette!(
            "SYMBOL_VISIBILITY.json schema.version 不支持：期望 v{CONE_SYMBOL_VISIBILITY_SCHEMA_VERSION}，但得到 v{}",
            file.schema.version
        ));
    }

    Ok(file)
}
