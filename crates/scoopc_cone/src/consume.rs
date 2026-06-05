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

use miette::{Diagnostic, IntoDiagnostic as _, Result, WrapErr as _, miette};
use thiserror::Error;

use scoop_project_model::{CONE_TOML_FILE_NAME, ConeId, ConeKind, ConeManifest};
use scoopc_ast as ast;
use scoopc_hir::cone_import::{
    CachedConeExtensionInfo, CachedConeFun, CachedConeImport, CachedConeNonPublicSymbol,
    CachedConeSymbolKind, CachedConeType, SyntheticSourceBuilder, inject_cached_cone_imports,
    last_segment, synthetic_decl_file,
};
use scoopc_hir::resolve::{
    BuiltinFunFlags, FunOverload, FunSig, ModifierSet, ParamSig, Symbol, SymbolKind, TypeParamSig,
    Visibility,
};
use scoopc_hir::typecheck::{TypeEnv, TypeSymbol, TypeSymbolKind};
use scoopc_source::SourceFile;
use scoopc_span::Span;

use super::annotations::{
    CONE_ANNOTATION_CLASSES_FILE_NAME, ConeAnnotationClassesFile, parse_annotation_classes_file,
};
use super::artifact::{ConeArtifact, ConeArtifactFrontendImport};
use super::pre_specialize::{
    CONE_PRE_SPECIALIZE_FILE_NAME, ConePreSpecializeFile, parse_pre_specialize_file,
};
use super::scoopir::{
    IrFunDeclKind, IrType, IrTypeDeclKind, IrVariance, SCOOPIR_SCHEMA_NAME, SCOOPIR_SCHEMA_VERSION,
    ScoopIrFile,
};
use super::visibility::{
    CONE_SYMBOL_VISIBILITY_FILE_NAME, ConeSymbolKind, ConeSymbolVisibilityFile,
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
#[error("api.scoopir schema.name 不匹配：期望 `{expected}`，但得到 `{found}`（归档：{archive}）")]
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
    index: &mut scoopc_hir::resolve::Index,
    env: &mut TypeEnv,
    decl_cone: ConeId,
    dep: &ConeArchiveApi,
) -> Result<()> {
    let import = build_cached_cone_import_from_archive(decl_cone, dep);
    inject_cached_cone_imports(index, env, std::slice::from_ref(&import))
}

/// Import frontend payloads from upstream cone artifacts into this frontend state.
pub fn import_upstream_artifacts(
    artifacts: &[&ConeArtifact],
    index: &mut scoopc_hir::resolve::Index,
    env: &mut TypeEnv,
) -> Result<()> {
    for (offset, artifact) in artifacts.iter().enumerate() {
        let cone_number = u32::try_from(offset + 1).map_err(|_| {
            miette!("too many upstream cone artifacts to assign a stable ConeId: {offset}")
        })?;
        inject_cone_artifact_frontend_import(index, env, ConeId::new(cone_number), artifact)?;
    }
    Ok(())
}

/// Inject the frontend import payload carried by a per-cone build artifact.
pub fn inject_cone_artifact_frontend_import(
    index: &mut scoopc_hir::resolve::Index,
    env: &mut TypeEnv,
    decl_cone: ConeId,
    artifact: &ConeArtifact,
) -> Result<()> {
    let import = build_cached_cone_import_from_artifact(decl_cone, artifact);
    inject_cached_cone_imports(index, env, std::slice::from_ref(&import))
}

/// Build a neutral `CachedConeImport` payload from a per-cone artifact.
///
/// This payload is injected into the consumer's `Index`/`TypeEnv` by the frontend; downstream
/// stages receive the resulting HIR semantic artifact instead of replaying this injection.
pub fn build_cached_cone_import_from_artifact(
    decl_cone: ConeId,
    artifact: &ConeArtifact,
) -> CachedConeImport {
    build_cached_cone_import_from_payload(
        decl_cone,
        &artifact.manifest.cone_name,
        &artifact.manifest.cone_version,
        artifact.manifest.cone_kind,
        &artifact.frontend_import,
    )
}

/// Build a neutral `CachedConeImport` payload from a `.cone` archive (legacy path).
pub fn build_cached_cone_import_from_archive(
    decl_cone: ConeId,
    dep: &ConeArchiveApi,
) -> CachedConeImport {
    let payload = ConeArtifactFrontendImport::new(
        dep.api.clone(),
        dep.annotation_classes.clone(),
        dep.symbol_visibility.clone(),
        dep.pre_specialize.clone(),
    );
    build_cached_cone_import_from_payload(
        decl_cone,
        &dep.manifest.cone.name,
        &dep.manifest.cone.version,
        dep.manifest.cone.kind,
        &payload,
    )
}

fn build_cached_cone_import_from_payload(
    decl_cone: ConeId,
    cone_name: &str,
    cone_version: &str,
    cone_kind: ConeKind,
    payload: &ConeArtifactFrontendImport,
) -> CachedConeImport {
    let decl_file = synthetic_decl_file(cone_name, cone_version);

    // NOTE: 这里用一个“合成 SourceFile”承载从 ScoopIR 反解出来的标识符文本，
    // 以便后续在 lowering 时可通过 span 切片拿到 segment/type param 名字。
    //
    // 约束：这些 span 并不对应真实源代码位置，因此诊断 label 仅用于稳定/可读性，不追求精确定位。
    let mut synth = SyntheticSourceBuilder::default();

    // 注解类元信息（v0：仅 cone-preserved）。
    let mut annotation_classes_by_fqn: std::collections::HashMap<
        &str,
        &super::annotations::ConeAnnotationClassEntry,
    > = std::collections::HashMap::new();
    if let Some(file) = payload.annotation_classes.as_ref() {
        for a in &file.annotations {
            annotation_classes_by_fqn.insert(a.fqn.as_str(), a);
        }
    }

    let mut types: Vec<CachedConeType> = Vec::with_capacity(payload.public_api.types.len());
    for ty in &payload.public_api.types {
        let fqn = ty.fqn.clone();
        let local = last_segment(&fqn).to_string();
        let local_span = synth.alloc(&local);

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
                        .filter_map(|t| {
                            scoopc_hir::typecheck::AnnotationTargetKind::from_variant_name(t)
                        })
                        .collect::<Vec<_>>()
                });
                let retention =
                    scoopc_hir::typecheck::AnnotationRetentionPolicy::parse(&meta.retention);
                let is_annotation_class = retention
                    == Some(scoopc_hir::typecheck::AnnotationRetentionPolicy::ConePreserved);
                (is_annotation_class, targets, retention)
            } else {
                (false, None, None)
            };

        let alias_of = if matches!(ty.kind, IrTypeDeclKind::TypeAlias) {
            ty.alias_of
                .as_ref()
                .map(|rhs| ir_type_to_type_ref(&mut synth, rhs))
        } else {
            None
        };

        types.push(CachedConeType {
            fqn: fqn.clone(),
            local: local.clone(),
            local_span,
            symbol: TypeSymbol {
                kind: type_kind,
                is_annotation_class,
                annotation_targets,
                annotation_retention,
                annotation_params: Vec::new(),
                is_interior_mutable: ty.is_interior_mutable,
                type_param_count: type_param_names.len(),
                eff_param: None,
                type_param_names,
                type_param_variances,
                where_constraints: Vec::new(),
                span: local_span,
                decl_file: decl_file.clone(),
            },
            is_enum: matches!(ty.kind, IrTypeDeclKind::Enum),
            alias_of,
        });
    }

    let mut funs: Vec<CachedConeFun> = Vec::with_capacity(payload.public_api.funs.len());
    for fun in &payload.public_api.funs {
        let fqn = fun.fqn.clone();
        let local = last_segment(&fqn).to_string();
        let local_span = synth.alloc(&local);

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
                bounds: Vec::new(),
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
                is_vararg: false,
            })
            .collect::<Vec<_>>();

        let return_ty = Some(ir_type_to_type_ref(&mut synth, &fun.return_ty));
        let effects = ir_effect_row_to_effect_row_expr(&mut synth, &fun.effects);

        let overload = FunOverload {
            symbol: Symbol {
                kind: SymbolKind::Fun,
                name: local.clone(),
                span: local_span,
                decl_file: decl_file.clone(),
                decl_cone,
                visibility: Visibility::Public,
                modifiers: ModifierSet::default(),
            },
            sig: FunSig {
                kind,
                receiver,
                type_params,
                eff_param: None,
                params,
                return_ty,
                effects,
                builtin_flags: BuiltinFunFlags::default(),
                where_clause: None,
            },
            has_body: false,
        };

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
        let extension = receiver_ty_fqn.map(|recv_fqn| {
            let pkg_prefix = fqn
                .rsplit_once('.')
                .map(|(p, _)| p.to_string())
                .unwrap_or_default();
            CachedConeExtensionInfo {
                pkg_prefix,
                receiver_ty_fqn: Some(recv_fqn),
                receiver_is_type_param: false,
            }
        });

        funs.push(CachedConeFun {
            fqn,
            overload,
            extension,
        });
    }

    let mut non_public_symbols: Vec<CachedConeNonPublicSymbol> = Vec::new();
    if let Some(vis) = payload.symbol_visibility.as_ref() {
        for sym in &vis.symbols {
            let visibility: Visibility = sym.visibility.into();
            if visibility == Visibility::Public {
                continue;
            }
            let local = last_segment(&sym.fqn).to_string();
            let local_span = synth.alloc(&local);
            let kind = match sym.kind {
                ConeSymbolKind::Type => CachedConeSymbolKind::Type,
                ConeSymbolKind::Value => CachedConeSymbolKind::Value,
                ConeSymbolKind::Fun => CachedConeSymbolKind::Fun,
            };
            non_public_symbols.push(CachedConeNonPublicSymbol {
                kind,
                fqn: sym.fqn.clone(),
                local,
                local_span,
                visibility,
            });
        }
    }

    let synthetic_source = SourceFile::new_virtual(decl_file.clone(), synth.finish());

    CachedConeImport {
        decl_cone,
        cone_kind,
        cone_name: cone_name.to_string(),
        cone_version: cone_version.to_string(),
        decl_file,
        synthetic_source,
        types,
        funs,
        non_public_symbols,
    }
}

fn ir_effect_row_to_effect_row_expr(
    synth: &mut SyntheticSourceBuilder,
    row: &super::scoopir::IrEffectRow,
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use scoop_project_model::StableConeKey;
    use scoopc_effect_facts::EffectFacts;
    use scoopc_hir_facts::HirFacts;
    use scoopc_lir::LateLoweredProgram;
    use scoopc_mir_facts::MirFacts;

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
        header.set_path(crate::CONE_API_SCOOPIR_FILE_NAME).unwrap();
        header.set_size(api_json.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        builder.append(&header, api_json).unwrap();
        builder.finish().unwrap();
    }

    // P1-T03：`.cone` consume 不再参与 active dependency flow；这些测试只覆盖保留的 future helper。
    #[test]
    fn future_archive_read_cone_api_scoopir_allows_current_version() {
        let dir = make_temp_dir("future_archive_read_cone_api_scoopir_allows_current_version");
        let cone_path = dir.join("dep.cone");

        let api_json = format!(
            r#"{{
  "schema": {{ "name": "{name}", "version": {version} }},
  "types": [],
  "funs": []
}}"#,
            name = crate::scoopir::SCOOPIR_SCHEMA_NAME,
            version = crate::scoopir::SCOOPIR_SCHEMA_VERSION,
        );
        write_cone_archive_with_api_scoopir(&cone_path, api_json.as_bytes());

        let api = read_cone_api_scoopir_from_archive(&cone_path).unwrap();
        assert_eq!(api.schema.version, crate::scoopir::SCOOPIR_SCHEMA_VERSION);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn future_archive_read_cone_api_scoopir_rejects_newer_version_with_stable_code() {
        let dir = make_temp_dir(
            "future_archive_read_cone_api_scoopir_rejects_newer_version_with_stable_code",
        );
        let cone_path = dir.join("dep.cone");

        let api_json = format!(
            r#"{{
  "schema": {{ "name": "{name}", "version": {version} }},
  "types": [],
  "funs": []
}}"#,
            name = crate::scoopir::SCOOPIR_SCHEMA_NAME,
            version = crate::scoopir::SCOOPIR_SCHEMA_VERSION + 1,
        );
        write_cone_archive_with_api_scoopir(&cone_path, api_json.as_bytes());

        let err = read_cone_api_scoopir_from_archive(&cone_path).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::cone::scoopir_schema_version_not_supported"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn artifact_frontend_import_injects_public_and_visibility_payload() {
        let artifact = ConeArtifact::with_parts(
            StableConeKey::new("dep", "1.0.0"),
            scoop_project_model::ConeKind::Lib,
            crate::artifact::ConeArtifactStageProducts::new(
                HirFacts::new(),
                MirFacts::new(),
                EffectFacts::new(),
                LateLoweredProgram::new(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                scoopc_types::TypeStore::new(),
            ),
            ConeArtifactFrontendImport::new(
                ScoopIrFile::new_v0(
                    vec![crate::scoopir::IrTypeDecl {
                        fqn: "dep.Token".to_owned(),
                        kind: crate::scoopir::IrTypeDeclKind::Struct,
                        type_params: Vec::new(),
                        is_interior_mutable: true,
                        alias_of: None,
                    }],
                    vec![crate::scoopir::IrFunDecl {
                        fqn: "dep.make_token".to_owned(),
                        kind: crate::scoopir::IrFunDeclKind::Regular,
                        type_params: Vec::new(),
                        receiver: None,
                        params: Vec::new(),
                        return_ty: crate::scoopir::IrType::Named {
                            fqn: "dep.Token".to_owned(),
                            args: Vec::new(),
                            eff: None,
                        },
                        effects: crate::scoopir::IrEffectRow::default(),
                    }],
                ),
                None,
                Some(ConeSymbolVisibilityFile::new_v0(vec![
                    crate::visibility::ConeSymbolVisibilityEntry {
                        kind: ConeSymbolKind::Fun,
                        fqn: "dep.hidden".to_owned(),
                        visibility: crate::visibility::ConeSymbolVisibility::Internal,
                    },
                ])),
                None,
            ),
            Vec::new(),
            crate::artifact::ConeArtifactFingerprints::default(),
        );
        let mut index = scoopc_hir::resolve::Index::default();
        let mut env = TypeEnv::default();

        inject_cone_artifact_frontend_import(&mut index, &mut env, ConeId::new(7), &artifact)
            .expect("inject artifact frontend import");

        assert!(env.nominal_is_interior_mutable("dep.Token"));
        assert!(index.by_fqn["dep.Token"].ty.is_some());
        assert_eq!(index.by_fqn["dep.make_token"].fun.len(), 1);
        assert_eq!(index.by_fqn["dep.hidden"].fun.len(), 1);
        assert_eq!(
            index.by_fqn["dep.hidden"].fun[0].symbol.visibility,
            Visibility::Internal
        );
    }
}
