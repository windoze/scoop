//! `.cone` 归档消费（读取）与下游注入（TODO T1105）。
//!
//! 本模块的目标是打通最小链路：
//! - 从 `.cone` 读取 `Cone.toml` 与 `api.scoopir`；
//! - 校验 ScoopIR schema（name/version）；
//! - 将依赖 cone 的 public API 注入下游：
//!   - `resolve::Index`：用于 import/名字解析；
//!   - `typecheck::TypeEnv`：用于 TypeRef lowering 与类型检查。
//!
//! 当前阶段限制：
//! - 支持读取 schema v0（向后兼容：新编译器可读旧版本；若遇到更高版本则报错）；
//! - 下游除注入 `api.scoopir` 的 public API 外，还会读取 `SYMBOL_VISIBILITY.json`：
//!   仅用于把依赖 cone 的非 public 符号以“不可见占位符”注入 resolver `Index`，
//!   以便在使用点给出稳定的 `scoop::resolve::not_visible`（TODO T0321b）。

use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result, miette};
use thiserror::Error;

use crate::ast;
use crate::resolve::{
    BuiltinFunFlags, ConeId, ExtensionFunSymbol, FunOverload, FunSig, ModifierSet, NamespacedSymbols,
    ParamSig, Symbol, SymbolKind, TypeParamSig, Visibility,
};
use crate::source::SourceFile;
use crate::span::Span;
use crate::typecheck::{TypeEnv, TypeSymbol, TypeSymbolKind};

use super::annotations::{
    ConeAnnotationClassesFile, CONE_ANNOTATION_CLASSES_FILE_NAME, parse_annotation_classes_file,
};
use super::manifest::{CONE_TOML_FILE_NAME, ConeManifest};
use super::pre_specialize::{
    ConePreSpecializeFile, CONE_PRE_SPECIALIZE_FILE_NAME, parse_pre_specialize_file,
};
use super::scoopir::{
    IrFunDeclKind, IrType, IrTypeDeclKind, IrVariance, SCOOPIR_SCHEMA_NAME, SCOOPIR_SCHEMA_VERSION,
    ScoopIrFile,
};
use super::visibility::{
    ConeSymbolKind, ConeSymbolVisibilityFile, CONE_SYMBOL_VISIBILITY_FILE_NAME,
    parse_symbol_visibility_file,
};

#[derive(Debug, Clone)]
pub struct ConeArchiveApi {
    pub path: PathBuf,
    pub manifest: ConeManifest,
    pub api: ScoopIrFile,
    /// 可选：注解类元信息（用于下游识别 `annotation class` 与 @Target/@Retention）。
    pub annotation_classes: Option<ConeAnnotationClassesFile>,
    /// 可选：非 public 符号可见性索引（用于 not_visible 诊断）。
    pub symbol_visibility: Option<ConeSymbolVisibilityFile>,
    /// 可选：pre-specialize 预编译实例索引（TODO T1108）。
    pub pre_specialize: Option<ConePreSpecializeFile>,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "api.scoopir schema.name 不匹配：期望 `{expected}`，但得到 `{found}`（归档：{archive}）"
)]
#[diagnostic(code(scoop::cone::scoopir_schema_name_mismatch))]
struct ScoopIrSchemaNameMismatch {
    expected: String,
    found: String,
    archive: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "api.scoopir schema.version 不支持：当前编译器最多支持 v{max_supported}，但得到 v{found}（归档：{archive}）"
)]
#[diagnostic(code(scoop::cone::scoopir_schema_version_not_supported))]
struct ScoopIrSchemaVersionNotSupported {
    max_supported: u32,
    found: u32,
    archive: String,
}

/// 从 `.cone` 读取并解析 `Cone.toml`（manifest）。
pub fn read_cone_manifest_from_archive(path: impl AsRef<Path>) -> Result<ConeManifest> {
    let path = path.as_ref();
    let bytes = super::read_cone_archive_entry(path, CONE_TOML_FILE_NAME)?;
    let text = String::from_utf8(bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("Cone.toml 必须为 UTF-8 文本（归档：{}）", path.display()))?;
    ConeManifest::parse_str(&text).wrap_err_with(|| {
        format!(
            "解析 Cone.toml 失败（归档：{}，条目：{CONE_TOML_FILE_NAME}）",
            path.display()
        )
    })
}

/// 从 `.cone` 读取并解析 `api.scoopir`（ScoopIR v0 JSON）。
pub fn read_cone_api_scoopir_from_archive(path: impl AsRef<Path>) -> Result<ScoopIrFile> {
    let path = path.as_ref();
    let bytes = super::read_cone_archive_entry(path, super::CONE_API_SCOOPIR_FILE_NAME)?;
    let api: ScoopIrFile = serde_json::from_slice(&bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("解析 api.scoopir 失败（归档：{}）", path.display()))?;

    if api.schema.name != SCOOPIR_SCHEMA_NAME {
        return Err(ScoopIrSchemaNameMismatch {
            expected: SCOOPIR_SCHEMA_NAME.to_string(),
            found: api.schema.name.clone(),
            archive: path.display().to_string(),
        }
        .into());
    }
    // v{api.schema.version} <= v{SCOOPIR_SCHEMA_VERSION}：允许向后兼容；
    // v{api.schema.version} >  v{SCOOPIR_SCHEMA_VERSION}：当前编译器无法读取（需升级编译器）。
    if api.schema.version > SCOOPIR_SCHEMA_VERSION {
        return Err(ScoopIrSchemaVersionNotSupported {
            max_supported: SCOOPIR_SCHEMA_VERSION,
            found: api.schema.version,
            archive: path.display().to_string(),
        }
        .into());
    }

    Ok(api)
}

/// 同时读取 `Cone.toml` 与 `api.scoopir`。
pub fn load_cone_archive_api(path: impl AsRef<Path>) -> Result<ConeArchiveApi> {
    let path = path.as_ref().to_path_buf();
    let manifest = read_cone_manifest_from_archive(&path)?;
    let api = read_cone_api_scoopir_from_archive(&path)?;
    let annotation_classes = read_cone_annotation_classes_from_archive(&path)?;
    let symbol_visibility = read_cone_symbol_visibility_from_archive(&path)?;
    let pre_specialize = read_cone_pre_specialize_from_archive(&path)?;
    Ok(ConeArchiveApi {
        path,
        manifest,
        api,
        annotation_classes,
        symbol_visibility,
        pre_specialize,
    })
}

/// 从 `.cone` 读取并解析 `ANNOTATION_CLASSES.json`（可选；用于注解类识别与 meta 信息）。
pub fn read_cone_annotation_classes_from_archive(
    path: impl AsRef<Path>,
) -> Result<Option<ConeAnnotationClassesFile>> {
    let path = path.as_ref();
    let Some(bytes) = super::try_read_cone_archive_entry(path, CONE_ANNOTATION_CLASSES_FILE_NAME)?
    else {
        return Ok(None);
    };

    let file = parse_annotation_classes_file(&bytes)?;
    Ok(Some(file))
}

/// 从 `.cone` 读取并解析 `SYMBOL_VISIBILITY.json`（可选；用于 not_visible 诊断）。
pub fn read_cone_symbol_visibility_from_archive(
    path: impl AsRef<Path>,
) -> Result<Option<ConeSymbolVisibilityFile>> {
    let path = path.as_ref();
    let Some(bytes) = super::try_read_cone_archive_entry(path, CONE_SYMBOL_VISIBILITY_FILE_NAME)?
    else {
        return Ok(None);
    };

    let file = parse_symbol_visibility_file(&bytes)?;
    Ok(Some(file))
}

/// 从 `.cone` 读取并解析 `PRE_SPECIALIZE.json`（可选；用于单态化缓存命中）。
pub fn read_cone_pre_specialize_from_archive(
    path: impl AsRef<Path>,
) -> Result<Option<ConePreSpecializeFile>> {
    let path = path.as_ref();
    let Some(bytes) = super::try_read_cone_archive_entry(path, CONE_PRE_SPECIALIZE_FILE_NAME)?
    else {
        return Ok(None);
    };

    let file = parse_pre_specialize_file(&bytes)?;
    Ok(Some(file))
}

/// 将一个依赖 cone 的 public API 注入到当前编译单元的 `Index` 与 `TypeEnv`。
///
/// - `decl_cone`：该依赖 cone 在当前编译单元中的 cone id（仅用于 internal 语义；public 不受影响）。
pub fn inject_cone_dependency_public_api(
    index: &mut crate::resolve::Index,
    env: &mut TypeEnv,
    decl_cone: ConeId,
    dep: &ConeArchiveApi,
) -> Result<()> {
    let decl_file = PathBuf::from(format!(
        "<cone:{}@{}>",
        dep.manifest.cone.name, dep.manifest.cone.version
    ));

    // NOTE: 这里用一个“合成 SourceFile”承载从 ScoopIR 反解出来的标识符文本，
    // 以便后续在 lowering 时可通过 span 切片拿到 segment/type param 名字。
    //
    // 约束：这些 span 并不对应真实源代码位置，因此诊断 label 仅用于稳定/可读性，不追求精确定位。
    let mut synth = SyntheticSourceBuilder::default();

    // 注解类元信息（v0：仅 cone-preserved）。
    let mut annotation_classes_by_fqn: std::collections::HashMap<&str, &super::annotations::ConeAnnotationClassEntry> =
        std::collections::HashMap::new();
    if let Some(file) = dep.annotation_classes.as_ref() {
        for a in &file.annotations {
            annotation_classes_by_fqn.insert(a.fqn.as_str(), a);
        }
    }

    // 1) types：注入 Index + TypeEnv。
    for ty in &dep.api.types {
        let fqn = ty.fqn.clone();
        let local = last_segment(&fqn);
        let local_span = synth.alloc(local);

        let type_kind = match ty.kind {
            IrTypeDeclKind::Class => TypeSymbolKind::Nominal(ast::TypeKind::Class),
            IrTypeDeclKind::Interface => TypeSymbolKind::Nominal(ast::TypeKind::Interface),
            IrTypeDeclKind::Struct => TypeSymbolKind::Nominal(ast::TypeKind::Struct),
            IrTypeDeclKind::Enum => TypeSymbolKind::Nominal(ast::TypeKind::Enum),
            IrTypeDeclKind::Effect => TypeSymbolKind::Nominal(ast::TypeKind::Effect),
            IrTypeDeclKind::TypeAlias => TypeSymbolKind::TypeAlias,
        };

        let type_param_names = ty
            .type_params
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        let type_param_variances = ty
            .type_params
            .iter()
            .map(|p| match p.variance {
                None => None,
                Some(IrVariance::In) => Some(ast::TypeParamVariance::In),
                Some(IrVariance::Out) => Some(ast::TypeParamVariance::Out),
            })
            .collect::<Vec<_>>();

        let (is_annotation_class, annotation_targets, annotation_retention) =
            if let Some(meta) = annotation_classes_by_fqn.get(fqn.as_str()) {
                let targets = meta.targets.as_ref().map(|ts| {
                    ts.iter()
                        .filter_map(|t| crate::typecheck::AnnotationTargetKind::from_variant_name(t))
                        .collect::<Vec<_>>()
                });
                let retention = crate::typecheck::AnnotationRetentionPolicy::parse(&meta.retention);
                let is_annotation_class =
                    retention == Some(crate::typecheck::AnnotationRetentionPolicy::ConePreserved);
                (is_annotation_class, targets, retention)
            } else {
                (false, None, None)
            };

        env.insert_external_type_symbol(
            fqn.clone(),
            TypeSymbol {
                kind: type_kind,
                is_annotation_class,
                annotation_targets,
                annotation_retention,
                annotation_params: Vec::new(),
                type_param_count: type_param_names.len(),
                eff_param: None,
                type_param_names,
                type_param_variances,
                where_constraints: Vec::new(),
                span: local_span,
                decl_file: decl_file.clone(),
            },
        )
        .map_err(miette::Report::new)
        .wrap_err_with(|| format!("注入依赖 cone type 符号失败：{fqn}"))?;

        inject_type_symbol_into_index(
            index,
            &fqn,
            local,
            local_span,
            &decl_file,
            decl_cone,
            matches!(ty.kind, IrTypeDeclKind::Enum),
        )?;
    }

    // 2) funs：注入 Index（TypeEnv 已在上面注入 types；函数本体不进入 TypeEnv）。
    for fun in &dep.api.funs {
        let fqn = fun.fqn.clone();
        let local = last_segment(&fqn);
        let local_span = synth.alloc(local);

        let kind = match fun.kind {
            IrFunDeclKind::Regular => ast::FunDeclKind::Regular,
            IrFunDeclKind::EffectOp => ast::FunDeclKind::EffectOp,
        };

        let type_params = fun
            .type_params
            .iter()
            .map(|name| TypeParamSig {
                name: name.clone(),
                name_span: synth.alloc(name),
            })
            .collect::<Vec<_>>();

        let receiver = fun
            .receiver
            .as_ref()
            .map(|ty| ir_type_to_type_ref(&mut synth, ty));

        let params = fun
            .params
            .iter()
            .map(|p| ParamSig {
                name: p.name.clone(),
                name_span: synth.alloc(&p.name),
                ty: Some(ir_type_to_type_ref(&mut synth, &p.ty)),
                has_default: false,
            })
            .collect::<Vec<_>>();

        let return_ty = Some(ir_type_to_type_ref(&mut synth, &fun.return_ty));
        let effects = ir_effect_row_to_effect_row_expr(&mut synth, &fun.effects);

        let overload = FunOverload {
            symbol: Symbol {
                kind: SymbolKind::Fun,
                name: local.to_string(),
                span: local_span,
                decl_file: decl_file.clone(),
                decl_cone,
                visibility: Visibility::Public,
                modifiers: ModifierSet::default(),
            },
            sig: FunSig {
                kind,
                is_const: false,
                receiver,
                type_params,
                eff_param: None,
                params,
                return_ty,
                effects,
                builtin_flags: BuiltinFunFlags::default(),
            },
            has_body: false,
        };

        let entry = index
            .by_fqn
            .entry(fqn.clone())
            .or_insert_with(NamespacedSymbols::default);
        entry.fun.push(overload);

        // T0322：跨包 extension 导入需要 resolver 能在依赖 cone 的 API 里发现 extension fun。
        //
        // 说明：
        // - `.cone` 侧我们通过 ScoopIR 的 `receiver` 字段来识别“扩展函数”；
        // - 当前阶段仅在 receiver 是名义类型（Named）时记录其 FQN，便于 resolver 做最小匹配：
        //   - receiver 完全相等，或 receiver 为 `scoop.core.Any`（通配）。
        let receiver_ty_fqn = fun.receiver.as_ref().and_then(|ty| match ty {
            IrType::Named { fqn, .. } => Some(fqn.clone()),
            _ => None,
        });
        if receiver_ty_fqn.is_some() {
            let pkg_prefix = fqn
                .rsplit_once('.')
                .map(|(p, _)| p.to_string())
                .unwrap_or_default();
            index.extension_funs.push(ExtensionFunSymbol {
                fqn: fqn.clone(),
                pkg_prefix,
                decl_cone,
                name: local.to_string(),
                receiver_ty_fqn,
            });
        }
    }

    // 2.5) 注入非 public 符号占位符：仅用于在使用点生成 not_visible 诊断。
    if let Some(vis) = &dep.symbol_visibility {
        inject_non_public_symbols_into_index(index, &mut synth, decl_cone, &decl_file, vis)?;
    }

    // 3) 最后注入合成 source，确保 lowering 能在 `decl_file` 上取到文本切片。
    env.insert_external_source(SourceFile::new_virtual(decl_file, synth.finish()));

    Ok(())
}

fn inject_non_public_symbols_into_index(
    index: &mut crate::resolve::Index,
    synth: &mut SyntheticSourceBuilder,
    decl_cone: ConeId,
    decl_file: &Path,
    file: &ConeSymbolVisibilityFile,
) -> Result<()> {
    // v0：`SYMBOL_VISIBILITY.json` 只应该包含非 public 项；这里做一层防御性过滤。
    for sym in &file.symbols {
        let visibility: Visibility = sym.visibility.into();
        if visibility == Visibility::Public {
            continue;
        }

        let fqn = sym.fqn.as_str();
        let local = last_segment(fqn);
        let local_span = synth.alloc(local);

        let entry = index
            .by_fqn
            .entry(fqn.to_string())
            .or_insert_with(NamespacedSymbols::default);

        match sym.kind {
            ConeSymbolKind::Type => {
                if entry.ty.is_some() {
                    continue;
                }
                entry.ty = Some(Symbol {
                    kind: SymbolKind::Type,
                    name: local.to_string(),
                    span: local_span,
                    decl_file: decl_file.to_path_buf(),
                    decl_cone,
                    visibility,
                    modifiers: ModifierSet::default(),
                });
            }
            ConeSymbolKind::Value => {
                if entry.value.is_some() {
                    continue;
                }
                entry.value = Some(Symbol {
                    kind: SymbolKind::Value,
                    name: local.to_string(),
                    span: local_span,
                    decl_file: decl_file.to_path_buf(),
                    decl_cone,
                    visibility,
                    modifiers: ModifierSet::default(),
                });
            }
            ConeSymbolKind::Fun => {
                // 若该 FQN 已有 public overload（来自 api.scoopir），则不额外注入不可见占位符，
                // 避免在后续阶段引入“public/hidden overload 混合”的模糊语义。
                if entry.fun.iter().any(|o| o.symbol.visibility == Visibility::Public) {
                    continue;
                }

                entry.fun.push(FunOverload {
                    symbol: Symbol {
                        kind: SymbolKind::Fun,
                        name: local.to_string(),
                        span: local_span,
                        decl_file: decl_file.to_path_buf(),
                        decl_cone,
                        visibility,
                        modifiers: ModifierSet::default(),
                    },
                    sig: FunSig {
                        kind: ast::FunDeclKind::Regular,
                        is_const: false,
                        receiver: None,
                        type_params: Vec::new(),
                        eff_param: None,
                        params: Vec::new(),
                        return_ty: None,
                        effects: None,
                        builtin_flags: BuiltinFunFlags::default(),
                    },
                    has_body: false,
                });
            }
        }
    }

    Ok(())
}

fn inject_type_symbol_into_index(
    index: &mut crate::resolve::Index,
    fqn: &str,
    local: &str,
    span: Span,
    decl_file: &Path,
    decl_cone: ConeId,
    also_value_namespace: bool,
) -> Result<()> {
    let entry = index
        .by_fqn
        .entry(fqn.to_string())
        .or_insert_with(NamespacedSymbols::default);

    if entry.ty.is_some() {
        return Err(miette!("Index 已存在同名 type 符号：{fqn}"));
    }
    entry.ty = Some(Symbol {
        kind: SymbolKind::Type,
        name: local.to_string(),
        span,
        decl_file: decl_file.to_path_buf(),
        decl_cone,
        visibility: Visibility::Public,
        modifiers: ModifierSet::default(),
    });

    // 与 source 索引保持一致：enum 同时引入同名 value symbol（便于限定名访问）。
    if also_value_namespace && entry.value.is_none() {
        entry.value = Some(Symbol {
            kind: SymbolKind::Value,
            name: local.to_string(),
            span,
            decl_file: decl_file.to_path_buf(),
            decl_cone,
            visibility: Visibility::Public,
            modifiers: ModifierSet::default(),
        });
    }

    Ok(())
}

#[derive(Default)]
struct SyntheticSourceBuilder {
    text: String,
}

impl SyntheticSourceBuilder {
    fn alloc(&mut self, s: &str) -> Span {
        let start = self.text.len();
        self.text.push_str(s);
        let end = self.text.len();
        self.text.push('\n');
        Span::new(start, end)
    }

    fn ident(&mut self, s: &str) -> ast::Ident {
        ast::Ident {
            span: self.alloc(s),
            text: None,
        }
    }

    fn finish(self) -> String {
        self.text
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("scoopc_{prefix}_{}_{}", std::process::id(), nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_cone_archive_with_api_scoopir(cone_path: &Path, api_json: &[u8]) {
        let file = std::fs::File::create(cone_path).unwrap();
        let mut builder = tar::Builder::new(file);

        let mut header = tar::Header::new_gnu();
        header.set_path(crate::cone::CONE_API_SCOOPIR_FILE_NAME).unwrap();
        header.set_size(api_json.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        builder.append(&header, api_json).unwrap();
        builder.finish().unwrap();
    }

    #[test]
    fn read_cone_api_scoopir_allows_current_version() {
        let dir = make_temp_dir("read_cone_api_scoopir_allows_current_version");
        let cone_path = dir.join("dep.cone");

        let api_json = format!(
            r#"{{
  "schema": {{ "name": "{name}", "version": {version} }},
  "types": [],
  "funs": []
}}"#,
            name = crate::cone::scoopir::SCOOPIR_SCHEMA_NAME,
            version = crate::cone::scoopir::SCOOPIR_SCHEMA_VERSION,
        );
        write_cone_archive_with_api_scoopir(&cone_path, api_json.as_bytes());

        let api = read_cone_api_scoopir_from_archive(&cone_path).unwrap();
        assert_eq!(api.schema.version, crate::cone::scoopir::SCOOPIR_SCHEMA_VERSION);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_cone_api_scoopir_rejects_newer_version_with_stable_code() {
        let dir = make_temp_dir("read_cone_api_scoopir_rejects_newer_version_with_stable_code");
        let cone_path = dir.join("dep.cone");

        let api_json = format!(
            r#"{{
  "schema": {{ "name": "{name}", "version": {version} }},
  "types": [],
  "funs": []
}}"#,
            name = crate::cone::scoopir::SCOOPIR_SCHEMA_NAME,
            version = crate::cone::scoopir::SCOOPIR_SCHEMA_VERSION + 1,
        );
        write_cone_archive_with_api_scoopir(&cone_path, api_json.as_bytes());

        let err = read_cone_api_scoopir_from_archive(&cone_path).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::cone::scoopir_schema_version_not_supported"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

fn last_segment(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

fn ir_effect_row_to_effect_row_expr(
    synth: &mut SyntheticSourceBuilder,
    row: &crate::cone::scoopir::IrEffectRow,
) -> Option<ast::EffectRowExpr> {
    if row.terms.is_empty() {
        return None;
    }

    let mut terms: Vec<ast::TypePath> = Vec::with_capacity(row.terms.len());
    for t in &row.terms {
        let path = match ir_type_to_type_ref(synth, t) {
            ast::TypeRef::Path(p) => p,
            _ => type_path_from_fqn(synth, "scoop.core.Any", Vec::new()),
        };
        terms.push(path);
    }

    Some(ast::EffectRowExpr {
        span: Span::new(0, 0),
        terms,
        closed: false,
    })
}

fn type_path_from_fqn(
    synth: &mut SyntheticSourceBuilder,
    fqn: &str,
    args: Vec<ast::TypeRef>,
) -> ast::TypePath {
    let segments = fqn
        .split('.')
        .filter(|s| !s.is_empty())
        .map(|seg| synth.ident(seg))
        .collect::<Vec<_>>();

    // 为保持 lowering 简单：TypePath.span 取第一个 segment 的 span（若为空则为 0..0）。
    let span = segments
        .first()
        .map(|id| id.span)
        .unwrap_or_else(|| Span::new(0, 0));

    ast::TypePath {
        span,
        segments,
        args,
    }
}

fn ir_type_to_type_ref(synth: &mut SyntheticSourceBuilder, ty: &IrType) -> ast::TypeRef {
    match ty {
        IrType::Named { fqn, args, eff } => {
            let mut out_args = args
                .iter()
                .map(|a| ir_type_to_type_ref(synth, a))
                .collect::<Vec<_>>();

            if let Some(row) = eff {
                // v0：IR 用 `eff: Option<IrEffectRow>` 表示 use-site effect row arg。
                // 在 AST 中它被建模为 “args 列表末尾的 `TypeRef::EffectRowArg`”。
                let row_terms = row
                    .terms
                    .iter()
                    .map(|t| match ir_type_to_type_ref(synth, t) {
                        ast::TypeRef::Path(p) => p,
                        _ => type_path_from_fqn(synth, "scoop.core.Any", Vec::new()),
                    })
                    .collect::<Vec<_>>();
                out_args.push(ast::TypeRef::EffectRowArg {
                    span: Span::new(0, 0),
                    row: ast::EffectRowExpr {
                        span: Span::new(0, 0),
                        terms: row_terms,
                        closed: false,
                    },
                });
            }

            ast::TypeRef::Path(type_path_from_fqn(synth, fqn, out_args))
        }
        IrType::Param { name } => {
            let span = synth.alloc(name);
            ast::TypeRef::Path(ast::TypePath {
                span,
                segments: vec![ast::Ident { span, text: None }],
                args: Vec::new(),
            })
        }
        IrType::Tuple { elements } => ast::TypeRef::Tuple(ast::TypeTuple {
            span: Span::new(0, 0),
            elements: elements
                .iter()
                .map(|e| ir_type_to_type_ref(synth, e))
                .collect(),
        }),
        IrType::Function {
            receiver,
            params,
            return_ty,
            effects,
        } => {
            let effects = ir_effect_row_to_effect_row_expr(synth, effects);
            ast::TypeRef::Function(ast::TypeFunction {
                span: Span::new(0, 0),
                receiver: receiver
                    .as_ref()
                    .map(|r| Box::new(ir_type_to_type_ref(synth, r))),
                params_span: Span::new(0, 0),
                params: params
                    .iter()
                    .map(|p| ir_type_to_type_ref(synth, p))
                    .collect(),
                return_ty: Box::new(ir_type_to_type_ref(synth, return_ty)),
                effects,
            })
        }
        IrType::Union { .. } => {
            // v0：AST TypeRef 暂无 union 语法节点；为保持下游可继续编译，
            // 这里把 union 降级为 `Any`（保守）。
            ast::TypeRef::Path(type_path_from_fqn(synth, "scoop.core.Any", Vec::new()))
        }
    }
}
