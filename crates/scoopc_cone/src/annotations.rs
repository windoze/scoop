//! `.cone` 依赖的“注解类元信息”（v0）。
//!
//! 背景（T1016b）：
//! - 注解类的 `@Retention` 决定该注解是否应在 `.cone` 边界上被保留；
//! - 下游在通过 `.cone` 依赖进行类型检查时，需要知道：
//!   - 某个 public 类型是否为 `annotation class`；
//!   - 其 `@Target`/`@Retention` 元信息（用于 use-site 合法性检查）。
//!
//! v0 约定：
//! - 仅导出 **ConePreserved**（`@Retention("cone")`）的 public annotation classes；
//! - `@Retention("local")`（以及未显式标记 retention 的注解类）视为 local-only，不导出；
//! - 注解参数签名（`annotation class A(val x: Int)`）暂不导出：下游将保守地拒绝带参数的跨包注解使用。

use std::collections::HashSet;

use miette::{Context as _, IntoDiagnostic as _, Result};
use serde::{Deserialize, Serialize};

use scoop_project_model::ConeId;
use scoopc_hir::resolve::{Index, IndexedFile, Visibility};
use scoopc_hir::session::Session;
use scoopc_hir::typecheck::{AnnotationRetentionPolicy, TypeEnv};
use scoopc_source::SourceFile;

/// `.cone` 内的注解类元信息文件名（v0 约定）。
pub const CONE_ANNOTATION_CLASSES_FILE_NAME: &str = "ANNOTATION_CLASSES.json";

pub const CONE_ANNOTATION_CLASSES_SCHEMA_NAME: &str = "scoop.cone.annotation_classes";
pub const CONE_ANNOTATION_CLASSES_SCHEMA_VERSION: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeAnnotationClassesSchema {
    pub name: String,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConeAnnotationClassEntry {
    /// 注解类的全限定名（FQN）。
    pub fqn: String,
    /// `@Target(...)`：None 表示未声明（默认允许全部目标）；Some([]) 表示显式空集合。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
    /// `@Retention("cone")`（v0：文件中只会出现 `cone`）。
    pub retention: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeAnnotationClassesFile {
    pub schema: ConeAnnotationClassesSchema,
    pub annotations: Vec<ConeAnnotationClassEntry>,
}

impl ConeAnnotationClassesFile {
    pub fn new_v0(mut annotations: Vec<ConeAnnotationClassEntry>) -> Self {
        annotations.sort_by(|a, b| a.fqn.cmp(&b.fqn));
        annotations.dedup_by(|a, b| a.fqn == b.fqn);
        Self {
            schema: ConeAnnotationClassesSchema {
                name: CONE_ANNOTATION_CLASSES_SCHEMA_NAME.to_string(),
                version: CONE_ANNOTATION_CLASSES_SCHEMA_VERSION,
            },
            annotations,
        }
    }
}

/// 生成 cone 的 “ConePreserved annotation classes” 列表（用于下游注入）。
///
/// 约定（v0）：
/// - sysroot cone=0，当前 cone=1；
/// - 仅输出 `decl_file` 属于 `sources` 的类型；
/// - 仅输出 `public` 类型；
/// - 仅输出 `annotation class` 且 `@Retention("cone")` 的类型。
pub fn collect_cone_preserved_annotation_classes_for_cone_sources(
    session: &Session,
    sources: &[SourceFile],
) -> Result<ConeAnnotationClassesFile> {
    let source_paths: HashSet<_> = sources.iter().map(|s| s.path().to_path_buf()).collect();

    // 1) parse sources → AST（Index/TypeEnv 构建需要 AST 引用）。
    let mut asts = Vec::with_capacity(sources.len());
    for source in sources {
        let ast = scoopc_ast::parser::parse_file(source).map_err(miette::Report::from)?;
        asts.push(ast);
    }
    // 2) build index：sysroot cone=0，当前 cone=1（与 ScoopIR 导出保持一致）。
    let mut indexed: Vec<IndexedFile<'_>> = Vec::new();
    for f in session.sysroot().index_files() {
        indexed.push(IndexedFile {
            cone: ConeId::new(0),
            cone_kind: if f.source.is_trusted_syslib() {
                scoop_project_model::ConeKind::Syslib
            } else {
                scoop_project_model::ConeKind::Lib
            },
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source, ast) in sources.iter().zip(asts.iter()) {
        indexed.push(IndexedFile {
            cone: ConeId::new(1),
            cone_kind: scoop_project_model::ConeKind::Lib,
            source,
            file: ast,
        });
    }

    let index = Index::build_with_cones(&indexed).map_err(miette::Report::from)?;

    // 3) build type env：用于读取注解类的 meta-annotations（@Target/@Retention）。
    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index).map_err(miette::Report::from)?;
    for (source, ast) in sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::from)?;
    }

    // 4) 从 index + env 收集 public annotation classes。
    let mut out: Vec<ConeAnnotationClassEntry> = Vec::new();
    for (fqn, ns) in &index.by_fqn {
        let Some(sym) = ns.ty.as_ref() else {
            continue;
        };
        if !source_paths.contains(sym.decl_file.as_path()) {
            continue;
        }
        if sym.visibility != Visibility::Public {
            continue;
        }

        let Some(ty) = env.type_symbol(fqn) else {
            continue;
        };
        if !ty.is_annotation_class {
            continue;
        }

        // v0：只有显式标记为 `cone` 的注解类才会跨 cone 导出。
        if ty.annotation_retention != Some(AnnotationRetentionPolicy::ConePreserved) {
            continue;
        }

        let targets = ty
            .annotation_targets
            .as_ref()
            .map(|v| v.iter().map(|t| t.as_str().to_string()).collect::<Vec<_>>());

        out.push(ConeAnnotationClassEntry {
            fqn: fqn.clone(),
            targets,
            retention: AnnotationRetentionPolicy::ConePreserved
                .as_str()
                .to_string(),
        });
    }

    Ok(ConeAnnotationClassesFile::new_v0(out))
}

/// Collect cone-preserved public annotation classes from an already built frontend state.
pub fn collect_cone_preserved_annotation_classes_from_index_env(
    sources: &[SourceFile],
    index: &Index,
    env: &TypeEnv,
) -> ConeAnnotationClassesFile {
    let source_paths: HashSet<_> = sources.iter().map(|s| s.path().to_path_buf()).collect();
    let mut out: Vec<ConeAnnotationClassEntry> = Vec::new();
    for (fqn, ns) in &index.by_fqn {
        let Some(sym) = ns.ty.as_ref() else {
            continue;
        };
        if !source_paths.contains(sym.decl_file.as_path()) || sym.visibility != Visibility::Public {
            continue;
        }

        let Some(ty) = env.type_symbol(fqn) else {
            continue;
        };
        if !ty.is_annotation_class {
            continue;
        }
        if ty.annotation_retention != Some(AnnotationRetentionPolicy::ConePreserved) {
            continue;
        }

        let targets = ty
            .annotation_targets
            .as_ref()
            .map(|v| v.iter().map(|t| t.as_str().to_string()).collect::<Vec<_>>());
        out.push(ConeAnnotationClassEntry {
            fqn: fqn.clone(),
            targets,
            retention: AnnotationRetentionPolicy::ConePreserved
                .as_str()
                .to_string(),
        });
    }

    ConeAnnotationClassesFile::new_v0(out)
}

pub fn parse_annotation_classes_file(bytes: &[u8]) -> Result<ConeAnnotationClassesFile> {
    let file: ConeAnnotationClassesFile = serde_json::from_slice(bytes)
        .into_diagnostic()
        .wrap_err("解析 ANNOTATION_CLASSES.json 失败")?;

    if file.schema.name != CONE_ANNOTATION_CLASSES_SCHEMA_NAME {
        return Err(miette::miette!(
            "ANNOTATION_CLASSES.json schema.name 不匹配：期望 `{CONE_ANNOTATION_CLASSES_SCHEMA_NAME}`，但得到 `{}`",
            file.schema.name
        ));
    }
    if file.schema.version != CONE_ANNOTATION_CLASSES_SCHEMA_VERSION {
        return Err(miette::miette!(
            "ANNOTATION_CLASSES.json schema.version 不支持：期望 v{CONE_ANNOTATION_CLASSES_SCHEMA_VERSION}，但得到 v{}",
            file.schema.version
        ));
    }

    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_json_surface_stays_semantic_and_path_free(json: &str) {
        for forbidden in [
            "TypeId(",
            "ClosureId(",
            "SourceId(",
            "ConeId(",
            "SymbolId(",
            "BasicBlockId(",
            "LocalId(",
            "SiteId(",
            "StepSchemaId(",
            "ContinuationSchemaId(",
            "CaseTag(",
            "source_path",
            "decl_span",
            "/Users/",
            env!("CARGO_MANIFEST_DIR"),
        ] {
            assert!(
                !json.contains(forbidden),
                "healthy ANNOTATION_CLASSES schema surface 不应泄漏 dense id/path 文本: {forbidden}\n{json}"
            );
        }
    }

    #[test]
    fn annotation_classes_json_baseline_stays_semantic_and_path_free() {
        let file = ConeAnnotationClassesFile::new_v0(vec![ConeAnnotationClassEntry {
            fqn: "a.Trace".to_string(),
            targets: Some(vec!["class".to_string(), "fun".to_string()]),
            retention: "cone".to_string(),
        }]);

        let json = serde_json::to_string_pretty(&file).unwrap();
        assert!(json.contains("\"fqn\": \"a.Trace\""));
        assert!(json.contains("\"retention\": \"cone\""));
        assert_json_surface_stays_semantic_and_path_free(&json);
    }
}
