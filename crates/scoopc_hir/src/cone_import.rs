//! 中性 cone import 注入（P10-T04-b）。
//!
//! 一个已经构建并被 cache hit 的 dependency cone 在前端会以 “artifact” 形式注入：
//! - `Index`：types/funs/extension funs/non-public 占位符 + cone-mapping（file_cones,
//!   file_cone_infos, cone_kinds）
//! - `TypeEnv`：type symbols / typealias RHS / 合成 SourceFile / `FileTypeContext`
//!
//! 历史上这些注入逻辑只发生在 `scoopc/frontend.rs` 这一处；但 `EffectFactsTypeContext::build`
//! 等下游 stage 会从 `compilation_sources` 独立重建 `Index`/`TypeEnv`，看不到 cached dep。
//! 本模块提供一个 **中性的** payload + helper，使得任何重建 Index/TypeEnv 的 stage 都能
//! 重放这次注入，从而让 cached dep 在所有下游 stage 都可见。
//!
//! 设计要点：
//! - **中性**：不依赖 `scoopc_cone` 的任何 wire schema（ScoopIR / SymbolVisibilityFile / …）。
//! - 数据本身使用 `ast::TypeRef` / `ast::EffectRowExpr` / `TypeSymbol` 等已经存在的中性类型。
//! - 由 `scoopc_cone::consume` 把 `.cone` artifact 翻译成本模块的 `CachedConeImport`，
//!   然后在所有需要重放的位置调用 `inject_cached_cone_imports`。

use std::path::{Path, PathBuf};

use miette::{Context as _, Result, miette};

use crate::ast;
use crate::cone::{ConeId, ConeInfo, ConeKind};
use crate::resolve::{
    ExtensionFunSymbol, FunOverload, Index, ModifierSet, Symbol, SymbolKind, Visibility,
};
use crate::source::SourceFile;
use crate::span::Span;
use crate::typecheck::{FileTypeContext, TypeEnv, TypeSymbol};

/// 一个 cached dep cone 在前端阶段产生的全部影响。
///
/// 由 `scoopc_cone::consume` 在加载 artifact 时构造一次，然后随 `FrontendOutput`
/// 透传给所有需要重建 Index/TypeEnv 的 stage（effect_facts/mir/rtti/...）。
#[derive(Debug, Clone)]
pub struct CachedConeImport {
    /// 该 cone 在当前编译单元内的 `ConeId`（用于 internal 可见性过滤）。
    pub decl_cone: ConeId,
    /// 该 cone 的 kind（lib/bin/syslib），来自 manifest。
    pub cone_kind: ConeKind,
    pub cone_name: String,
    pub cone_version: String,
    /// 合成 “声明源” 的虚拟路径：`<cone:name@version>`。
    /// 所有该 cone 注入符号的 `decl_file` 都指向这里。
    pub decl_file: PathBuf,
    /// 合成 SourceFile，用于支持 lowering 通过 span 切片取回标识符文本。
    pub synthetic_source: SourceFile,
    pub types: Vec<CachedConeType>,
    pub funs: Vec<CachedConeFun>,
    pub non_public_symbols: Vec<CachedConeNonPublicSymbol>,
}

/// 单个被注入的 type 符号。
#[derive(Debug, Clone)]
pub struct CachedConeType {
    pub fqn: String,
    pub local: String,
    pub local_span: Span,
    pub symbol: TypeSymbol,
    /// 是否为 `enum`：决定是否要把同名 value 也注入到 Index（与 source 索引语义一致）。
    pub is_enum: bool,
    /// 若该 type 是 typealias，这里给出 RHS。
    pub alias_of: Option<ast::TypeRef>,
}

/// 单个被注入的 fun 符号（public）。
#[derive(Debug, Clone)]
pub struct CachedConeFun {
    pub fqn: String,
    pub overload: FunOverload,
    /// 若该 fun 是扩展函数，这里给出补充信息以登记到 `Index::extension_funs`。
    pub extension: Option<CachedConeExtensionInfo>,
}

#[derive(Debug, Clone)]
pub struct CachedConeExtensionInfo {
    pub pkg_prefix: String,
    pub receiver_ty_fqn: Option<String>,
    pub receiver_is_type_param: bool,
}

/// 中性形式的非 public 符号 kind（避免依赖 `scoopc_cone::visibility::ConeSymbolKind`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedConeSymbolKind {
    Type,
    Value,
    Fun,
}

/// 单个被注入的 “不可见占位符” 符号。
///
/// 用途：使用点的 `not_visible` 诊断（T0321b）能看到一个稳定的 `Symbol`。
#[derive(Debug, Clone)]
pub struct CachedConeNonPublicSymbol {
    pub kind: CachedConeSymbolKind,
    pub fqn: String,
    pub local: String,
    pub local_span: Span,
    pub visibility: Visibility,
}

/// 一个文本 buffer + span 分配器，用于把识别符串字符串化到合成 SourceFile 中。
///
/// 设计：把所有标识符按出现顺序追加到 `text`，并为每段返回 `[start, end)` 的 `Span`。
/// 这样 lowering 阶段在 span 上做 `source.slice(span)` 时仍能拿到原文。
#[derive(Default, Debug)]
pub struct SyntheticSourceBuilder {
    text: String,
}

impl SyntheticSourceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加 `s` 到 buffer，并返回它的 span。会自动追加一个换行做分隔。
    pub fn alloc(&mut self, s: &str) -> Span {
        let start = self.text.len();
        self.text.push_str(s);
        let end = self.text.len();
        self.text.push('\n');
        Span::new(start, end)
    }

    /// 等价于 `alloc(s)` + 包装成 `ast::Ident`（text 字段保留为 None；text 通过 span 切片获取）。
    pub fn ident(&mut self, s: &str) -> ast::Ident {
        ast::Ident {
            span: self.alloc(s),
            text: None,
        }
    }

    pub fn finish(self) -> String {
        self.text
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

/// 取 FQN 的最后一段（`a.b.C` -> `C`）。无 `.` 时返回原串。
pub fn last_segment(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

/// 构造 cached cone import 的 `decl_file` 路径：`<cone:name@version>`。
pub fn synthetic_decl_file(cone_name: &str, cone_version: &str) -> PathBuf {
    PathBuf::from(format!("<cone:{cone_name}@{cone_version}>"))
}

/// 把 `imports` 中的所有 cached dep cone 信息重放进 `index` 与 `env`。
///
/// 调用语义：
/// - 同一个 `decl_file` 的 `FileTypeContext` 与 cone-mapping 是 **first-write-wins**
///   （重复调用是幂等的，例如 effect_facts stage 在已注入的 frontend 上再注入也安全）；
/// - type symbol 重复注入会返回错误（与 frontend 阶段保持一致）；
/// - fun overload 会**累加**到 `by_fqn[fqn].fun`；
/// - 非 public 占位符若已存在同名 type/value/public-fun 则跳过。
///
/// 调用方应保证：
/// - 每个 `CachedConeImport` 的 `decl_file` 唯一；
/// - 不会在同一个 Index/TypeEnv 上对同一个 cached cone 重复注入。
pub fn inject_cached_cone_imports(
    index: &mut Index,
    env: &mut TypeEnv,
    imports: &[CachedConeImport],
) -> Result<()> {
    for import in imports {
        inject_cached_cone_import(index, env, import)?;
    }
    Ok(())
}

fn inject_cached_cone_import(
    index: &mut Index,
    env: &mut TypeEnv,
    import: &CachedConeImport,
) -> Result<()> {
    let decl_file = import.decl_file.as_path();
    let decl_cone = import.decl_cone;

    // 1) types：注入 Index + TypeEnv。
    for type_decl in &import.types {
        env.insert_external_type_symbol(type_decl.fqn.clone(), type_decl.symbol.clone())
            .map_err(miette::Report::new)
            .wrap_err_with(|| format!("注入依赖 cone type 符号失败：{}", type_decl.fqn))?;

        if let Some(rhs) = type_decl.alias_of.as_ref() {
            env.insert_external_type_alias(
                type_decl.fqn.clone(),
                decl_file.to_path_buf(),
                type_decl.local_span,
                rhs.clone(),
            );
        }

        inject_type_symbol_into_index(
            index,
            &type_decl.fqn,
            &type_decl.local,
            type_decl.local_span,
            decl_file,
            decl_cone,
            type_decl.is_enum,
        )?;
    }

    // 2) funs：注入 Index（TypeEnv 已在上面注入 types；函数本体不进入 TypeEnv）。
    for fun_decl in &import.funs {
        let entry = index.by_fqn.entry(fun_decl.fqn.clone()).or_default();
        entry.fun.push(fun_decl.overload.clone());

        if let Some(ext) = fun_decl.extension.as_ref() {
            index.extension_funs.push(ExtensionFunSymbol {
                fqn: fun_decl.fqn.clone(),
                pkg_prefix: ext.pkg_prefix.clone(),
                decl_cone,
                name: last_segment(&fun_decl.fqn).to_string(),
                receiver_ty_fqn: ext.receiver_ty_fqn.clone(),
                receiver_is_type_param: ext.receiver_is_type_param,
            });
        }
    }

    // 2.5) non-public 占位符（用于 not_visible 诊断）。
    for sym in &import.non_public_symbols {
        if sym.visibility == Visibility::Public {
            continue;
        }
        inject_non_public_symbol_into_index(index, decl_cone, decl_file, sym);
    }

    // 3) 合成 SourceFile + FileTypeContext + cone-mapping。
    env.insert_external_source(import.synthetic_source.clone());
    env.insert_external_file_type_context(
        decl_file.to_path_buf(),
        FileTypeContext {
            pkg_prefix: String::new(),
            imports: Default::default(),
            cone: ConeInfo {
                id: decl_cone,
                kind: import.cone_kind,
            },
        },
    );
    index.register_external_cone_decl_file(decl_file.to_path_buf(), decl_cone, import.cone_kind);

    Ok(())
}

fn inject_type_symbol_into_index(
    index: &mut Index,
    fqn: &str,
    local: &str,
    span: Span,
    decl_file: &Path,
    decl_cone: ConeId,
    also_value_namespace: bool,
) -> Result<()> {
    let entry = index.by_fqn.entry(fqn.to_string()).or_default();

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

fn inject_non_public_symbol_into_index(
    index: &mut Index,
    decl_cone: ConeId,
    decl_file: &Path,
    sym: &CachedConeNonPublicSymbol,
) {
    use crate::resolve::{BuiltinFunFlags, FunSig};

    let entry = index.by_fqn.entry(sym.fqn.clone()).or_default();

    match sym.kind {
        CachedConeSymbolKind::Type => {
            if entry.ty.is_some() {
                return;
            }
            entry.ty = Some(Symbol {
                kind: SymbolKind::Type,
                name: sym.local.clone(),
                span: sym.local_span,
                decl_file: decl_file.to_path_buf(),
                decl_cone,
                visibility: sym.visibility,
                modifiers: ModifierSet::default(),
            });
        }
        CachedConeSymbolKind::Value => {
            if entry.value.is_some() {
                return;
            }
            entry.value = Some(Symbol {
                kind: SymbolKind::Value,
                name: sym.local.clone(),
                span: sym.local_span,
                decl_file: decl_file.to_path_buf(),
                decl_cone,
                visibility: sym.visibility,
                modifiers: ModifierSet::default(),
            });
        }
        CachedConeSymbolKind::Fun => {
            // 已存在 public overload 时跳过，避免 public/hidden mixed overload 模糊语义。
            if entry
                .fun
                .iter()
                .any(|o| o.symbol.visibility == Visibility::Public)
            {
                return;
            }

            entry.fun.push(FunOverload {
                symbol: Symbol {
                    kind: SymbolKind::Fun,
                    name: sym.local.clone(),
                    span: sym.local_span,
                    decl_file: decl_file.to_path_buf(),
                    decl_cone,
                    visibility: sym.visibility,
                    modifiers: ModifierSet::default(),
                },
                sig: FunSig {
                    kind: ast::FunDeclKind::Regular,
                    receiver: None,
                    type_params: Vec::new(),
                    eff_param: None,
                    params: Vec::new(),
                    return_ty: None,
                    effects: None,
                    builtin_flags: BuiltinFunFlags::default(),
                    where_clause: None,
                },
                has_body: false,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::cone::{ConeId, ConeKind};
    use crate::resolve::{
        BuiltinFunFlags, FunSig, Index, ModifierSet, Symbol, SymbolKind, Visibility,
    };
    use crate::source::SourceFile;
    use crate::typecheck::{TypeEnv, TypeSymbol, TypeSymbolKind};

    fn make_type(fqn: &str, kind: ast::TypeKind, decl_file: PathBuf, span: Span) -> CachedConeType {
        CachedConeType {
            fqn: fqn.to_string(),
            local: last_segment(fqn).to_string(),
            local_span: span,
            symbol: TypeSymbol {
                kind: TypeSymbolKind::Nominal(kind),
                is_annotation_class: false,
                annotation_targets: None,
                annotation_retention: None,
                annotation_params: Vec::new(),
                type_param_count: 0,
                eff_param: None,
                type_param_names: Vec::new(),
                type_param_variances: Vec::new(),
                where_constraints: Vec::new(),
                span,
                decl_file,
            },
            is_enum: matches!(kind, ast::TypeKind::Enum),
            alias_of: None,
        }
    }

    fn make_fun(fqn: &str, decl_cone: ConeId, decl_file: PathBuf, span: Span) -> CachedConeFun {
        CachedConeFun {
            fqn: fqn.to_string(),
            overload: FunOverload {
                symbol: Symbol {
                    kind: SymbolKind::Fun,
                    name: last_segment(fqn).to_string(),
                    span,
                    decl_file,
                    decl_cone,
                    visibility: Visibility::Public,
                    modifiers: ModifierSet::default(),
                },
                sig: FunSig {
                    kind: ast::FunDeclKind::Regular,
                    receiver: None,
                    type_params: Vec::new(),
                    eff_param: None,
                    params: Vec::new(),
                    return_ty: None,
                    effects: None,
                    builtin_flags: BuiltinFunFlags::default(),
                    where_clause: None,
                },
                has_body: false,
            },
            extension: None,
        }
    }

    fn fixture_import() -> CachedConeImport {
        let mut synth = SyntheticSourceBuilder::new();
        let token_span = synth.alloc("Token");
        let make_token_span = synth.alloc("make_token");
        let hidden_span = synth.alloc("hidden");
        let decl_file = synthetic_decl_file("dep", "1.0.0");
        let synthetic_source = SourceFile::new_virtual(decl_file.clone(), synth.finish());
        let decl_cone = ConeId::new(7);

        CachedConeImport {
            decl_cone,
            cone_kind: ConeKind::Lib,
            cone_name: "dep".to_string(),
            cone_version: "1.0.0".to_string(),
            decl_file: decl_file.clone(),
            synthetic_source,
            types: vec![make_type(
                "dep.Token",
                ast::TypeKind::Struct,
                decl_file.clone(),
                token_span,
            )],
            funs: vec![make_fun(
                "dep.make_token",
                decl_cone,
                decl_file.clone(),
                make_token_span,
            )],
            non_public_symbols: vec![CachedConeNonPublicSymbol {
                kind: CachedConeSymbolKind::Fun,
                fqn: "dep.hidden".to_string(),
                local: "hidden".to_string(),
                local_span: hidden_span,
                visibility: Visibility::Internal,
            }],
        }
    }

    #[test]
    fn inject_populates_index_and_typeenv() {
        let import = fixture_import();
        let mut index = Index::default();
        let mut env = TypeEnv::default();

        inject_cached_cone_imports(&mut index, &mut env, std::slice::from_ref(&import))
            .expect("inject");

        // type symbol in env
        assert!(env.type_symbol("dep.Token").is_some());
        // type symbol in index
        assert!(index.by_fqn["dep.Token"].ty.is_some());
        // fun overload in index
        assert_eq!(index.by_fqn["dep.make_token"].fun.len(), 1);
        // non-public placeholder in index
        assert_eq!(index.by_fqn["dep.hidden"].fun.len(), 1);
        assert_eq!(
            index.by_fqn["dep.hidden"].fun[0].symbol.visibility,
            Visibility::Internal
        );

        // FileTypeContext for synthetic decl file is now populated — this is the bit
        // that previously was missing and caused EffectFactsTypeContext::build to fail.
        let ctx = env
            .file_type_context(&import.decl_file)
            .expect("file_type_context for synthetic decl file");
        assert_eq!(ctx.cone.id, import.decl_cone);
        assert_eq!(ctx.cone.kind, ConeKind::Lib);
        assert_eq!(ctx.pkg_prefix, "");

        // Index 也登记了 cone-mapping。
        let info = index.cone_info(import.decl_cone);
        assert_eq!(info.kind, ConeKind::Lib);
    }

    #[test]
    fn injected_symbols_carry_dep_cone_attribution_for_downstream_stages() {
        // P10-T04-b regression: effect_facts / mir / cone scoopir-export 等下游 stage 在
        // 重建 Index/TypeEnv 后会按 `decl_cone` / `decl_file` / `visibility` 做 surface
        // contract 推断。本测试锁定注入后这些字段不会被退化为 consumer 自身或丢失。
        let import = fixture_import();
        let mut index = Index::default();
        let mut env = TypeEnv::default();

        inject_cached_cone_imports(&mut index, &mut env, std::slice::from_ref(&import))
            .expect("inject");

        // Public type symbol: decl_cone & decl_file 必须指向 dep 自身，否则 internal 可见性
        // 过滤会把 dep type 误判为 consumer 内部符号。
        let token = index.by_fqn["dep.Token"]
            .ty
            .as_ref()
            .expect("dep.Token type symbol");
        assert_eq!(token.decl_cone, import.decl_cone);
        assert_eq!(token.decl_file, import.decl_file);
        assert_eq!(token.visibility, Visibility::Public);

        // Public fun overload: 同上，effect_facts 的 surface_callable_contract 会按
        // overload.symbol.decl_cone / decl_file 解析 callable owner。
        let make_token = &index.by_fqn["dep.make_token"].fun[0];
        assert_eq!(make_token.symbol.decl_cone, import.decl_cone);
        assert_eq!(make_token.symbol.decl_file, import.decl_file);
        assert_eq!(make_token.symbol.visibility, Visibility::Public);
        assert!(
            !make_token.has_body,
            "cached dep fun 应保持 has_body=false（body 不进入 consumer pipeline）"
        );

        // Non-public 占位符（用于 not_visible 诊断）必须保留原始 Internal 可见性，
        // 不被 inject 路径升格为 Public。
        let hidden = &index.by_fqn["dep.hidden"].fun[0];
        assert_eq!(hidden.symbol.decl_cone, import.decl_cone);
        assert_eq!(hidden.symbol.decl_file, import.decl_file);
        assert_eq!(hidden.symbol.visibility, Visibility::Internal);
    }

    #[test]
    fn injecting_into_already_typechecked_index_extends_dep_visibility() {
        // 模拟 `EffectFactsTypeContext::build` 的实际场景：先从 compilation_sources 重建
        // Index/TypeEnv（这里用 `Default` 占位 + 一个无关符号占位 consumer 已 typecheck 过的
        // 状态），再 `inject_cached_cone_imports` 把 cached dep 重放进来。
        // 注入后 dep 公共 fun / type 必须可见，且原有 consumer 符号不被破坏。
        let import = fixture_import();
        let mut index = Index::default();
        let mut env = TypeEnv::default();

        // 占位：consumer 自身的 symbol（不重名于 dep），用于验证不会被注入路径意外覆盖。
        index.by_fqn.insert(
            "consumer.local".to_string(),
            crate::resolve::NamespacedSymbols {
                ty: None,
                value: Some(Symbol {
                    kind: SymbolKind::Value,
                    name: "local".to_string(),
                    span: Span::new(0, 0),
                    decl_file: PathBuf::from("/tmp/consumer.scoop"),
                    decl_cone: ConeId::new(1),
                    visibility: Visibility::Internal,
                    modifiers: ModifierSet::default(),
                }),
                fun: Vec::new(),
            },
        );

        inject_cached_cone_imports(&mut index, &mut env, std::slice::from_ref(&import))
            .expect("inject must succeed against an already-populated Index/TypeEnv");

        // dep 符号现在应可见。
        assert!(
            env.type_symbol("dep.Token").is_some(),
            "dep type 必须在 inject 后可见于 TypeEnv"
        );
        assert_eq!(
            index.by_fqn["dep.make_token"].fun.len(),
            1,
            "dep public fun 必须在 inject 后可见于 Index"
        );
        // consumer 自身的占位符号没有被覆盖。
        let consumer_local = index.by_fqn["consumer.local"]
            .value
            .as_ref()
            .expect("consumer 占位符号应当保留");
        assert_eq!(consumer_local.decl_cone, ConeId::new(1));
        // FileTypeContext 也已注入（这是 effect_facts surface contract 推断的最关键 bit）。
        assert!(env.file_type_context(&import.decl_file).is_some());
    }

    #[test]
    fn duplicate_decl_file_is_idempotent_for_file_type_context() {
        let import = fixture_import();
        let mut index = Index::default();
        let mut env = TypeEnv::default();

        // 第一次注入。
        inject_cached_cone_imports(&mut index, &mut env, std::slice::from_ref(&import))
            .expect("first inject");

        // 删除 type 后第二次注入，应不会 panic（cone-mapping / FileTypeContext 应是 first-write-wins）。
        // 这里我们只验证 file_type_context 调用 idempotent；type 重复注入会报错由其它测试覆盖。
        env.insert_external_file_type_context(
            import.decl_file.clone(),
            FileTypeContext {
                pkg_prefix: "alt".to_string(),
                imports: Default::default(),
                cone: ConeInfo {
                    id: ConeId::new(99),
                    kind: ConeKind::Bin,
                },
            },
        );
        let ctx = env.file_type_context(&import.decl_file).unwrap();
        assert_eq!(ctx.pkg_prefix, "");
        assert_eq!(ctx.cone.id, import.decl_cone);
    }
}
