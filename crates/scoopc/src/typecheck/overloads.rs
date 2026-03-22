//! 重载声明冲突诊断（T0457）。
//!
//! 目标：
//! - 在 typecheck 阶段（不进入函数体）提前诊断“永远无法在调用点消歧”的 overload 集：
//!   - 完全相同签名（重复定义）
//!   - 仅返回类型不同（返回类型不参与重载决议）
//!   - 默认参数导致的不可区分（例如 `f(x:Int)` 与 `f(x:Int, y:Int=0)`）
//! - 当前实现先覆盖：
//!   - 顶层/成员 `fun`
//!   - class 的 primary/secondary constructors
//! - 先按“位置实参（positional call）”做最小分析：仅考虑尾部默认参数可被省略的调用形态。
//!   更完整的 named args / 中间省略规则由后续任务补齐（与 T1305/T1306 联动）。

use std::collections::HashMap;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{ImportTable, Index};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, TypeId, TypeStore};

use super::lower::{TypeLowerError, TypeLowering};
use super::TypeEnv;

#[derive(Debug, Error, Diagnostic)]
pub enum OverloadDeclError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLower(#[from] TypeLowerError),

    #[error("重载签名冲突：{fqn}（{reason}）")]
    #[diagnostic(code(scoop::typecheck::overload_conflict))]
    Conflict {
        fqn: String,
        reason: String,
        #[label("冲突声明在这里")]
        conflict: miette::SourceSpan,
        #[label("第一次声明在这里")]
        previous: miette::SourceSpan,
    },
}

#[derive(Debug, Clone)]
struct ParamInfo {
    ty: TypeId,
    has_default: bool,
}

#[derive(Debug, Clone)]
struct FunSigInfo {
    receiver: Option<TypeId>,
    params: Vec<ParamInfo>,
    return_ty: Option<TypeId>,
}

#[derive(Debug, Clone)]
struct FunDeclInfo {
    name_span: Span,
    sig: FunSigInfo,
}

#[derive(Debug, Clone)]
struct CtorSigInfo {
    params: Vec<ParamInfo>,
}

#[derive(Debug, Clone)]
struct CtorDeclInfo {
    span: Span,
    sig: CtorSigInfo,
}

/// 检查当前文件中的重载声明是否存在“必然冲突”的情况。
///
/// 说明：
/// - 该 pass 需要在 `check_file_type_refs` 之后运行，确保签名里的类型引用都可 lowering；
/// - 当前实现只分析“位置实参 + 尾部默认参数省略”的可调用性。
pub fn check_file_overload_conflicts(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), OverloadDeclError> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);

    let pkg_prefix = package_prefix(source, file.package.as_ref());

    let mut funs_by_fqn: HashMap<String, Vec<FunDeclInfo>> = HashMap::new();
    let mut ctors_by_type: HashMap<String, Vec<CtorDeclInfo>> = HashMap::new();

    collect_items(
        source,
        &file.items,
        &pkg_prefix,
        &mut lower,
        &mut funs_by_fqn,
        &mut ctors_by_type,
    )?;

    // 先检查函数 overload set。
    for (fqn, decls) in funs_by_fqn {
        check_fun_overload_set(&fqn, &decls)?;
    }

    // 再检查构造器 overload set（按宿主 type FQN 分组）。
    for (type_fqn, decls) in ctors_by_type {
        check_ctor_overload_set(&type_fqn, &decls)?;
    }

    Ok(())
}

fn collect_items(
    source: &SourceFile,
    items: &[ast::Item],
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    funs_by_fqn: &mut HashMap<String, Vec<FunDeclInfo>>,
    ctors_by_type: &mut HashMap<String, Vec<CtorDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    for item in items {
        match item {
            ast::Item::TypeAlias(_ta) => {}
            ast::Item::Fun(fun) => collect_fun_decl(source, fun, prefix, lower, funs_by_fqn)?,
            ast::Item::ExtensionProperty(_p) => {}
            ast::Item::Val(_v) => {}
            ast::Item::Type(ty) => collect_type_decl(source, ty, prefix, lower, funs_by_fqn, ctors_by_type)?,
            ast::Item::Object(obj) => {
                collect_object_decl(source, obj, prefix, lower, funs_by_fqn, ctors_by_type)?
            }
        }
    }
    Ok(())
}

fn collect_type_decl(
    source: &SourceFile,
    ty: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    funs_by_fqn: &mut HashMap<String, Vec<FunDeclInfo>>,
    ctors_by_type: &mut HashMap<String, Vec<CtorDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    let name = source.slice(ty.name.span).to_string();
    let type_fqn = join_prefix(prefix, &name);

    // class/type 的 type params 进入作用域，供 ctor/member signatures lowering。
    lower.push_type_params(&ty.type_params);
    let ty_eff_binding = if let Some(eff_param) = &ty.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
            None => crate::ty::EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    if let Some(primary) = &ty.primary_ctor {
        collect_ctor_decl(
            source,
            &type_fqn,
            primary.params_span,
            &primary.params,
            lower,
            ctors_by_type,
        )?;
    }

    if let Some(body) = &ty.body {
        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(_v) => {}
                ast::TypeMember::Property(_p) => {}
                ast::TypeMember::InitBlock(_b) => {}
                ast::TypeMember::SecondaryCtor(ctor) => {
                    collect_ctor_decl(
                        source,
                        &type_fqn,
                        ctor.span,
                        &ctor.params,
                        lower,
                        ctors_by_type,
                    )?;
                }
                ast::TypeMember::Fun(fun) => {
                    collect_fun_decl(source, fun, &type_fqn, lower, funs_by_fqn)?
                }
                ast::TypeMember::Type(nested) => collect_type_decl(
                    source,
                    nested,
                    &type_fqn,
                    lower,
                    funs_by_fqn,
                    ctors_by_type,
                )?,
                ast::TypeMember::Object(obj) => collect_object_decl(
                    source,
                    obj,
                    &type_fqn,
                    lower,
                    funs_by_fqn,
                    ctors_by_type,
                )?,
            }
        }
    }

    if ty_eff_binding {
        lower.pop_effect_row_param_binding();
    }
    lower.pop_type_params(&ty.type_params);

    Ok(())
}

fn collect_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    funs_by_fqn: &mut HashMap<String, Vec<FunDeclInfo>>,
    ctors_by_type: &mut HashMap<String, Vec<CtorDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    let obj_name = match &obj.name {
        Some(id) => source.slice(id.span).to_string(),
        None => {
            if !matches!(obj.kind, ast::ObjectKind::Companion) {
                // parser 会拒绝 `object { ... }`，这里作为防御性兜底。
                return Ok(());
            }
            "Companion".to_string()
        }
    };

    let obj_fqn = join_prefix(prefix, &obj_name);

    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(_v) => {}
            ast::TypeMember::Property(_p) => {}
            ast::TypeMember::InitBlock(_b) => {}
            ast::TypeMember::SecondaryCtor(ctor) => {
                // object 内也允许 constructor 语法节点（与 class 共享 TypeBody 结构），
                // 但它在语义上无效；当前阶段忽略即可。
                let _ = ctor;
            }
            ast::TypeMember::Fun(fun) => collect_fun_decl(source, fun, &obj_fqn, lower, funs_by_fqn)?,
            ast::TypeMember::Type(nested) => collect_type_decl(
                source,
                nested,
                &obj_fqn,
                lower,
                funs_by_fqn,
                ctors_by_type,
            )?,
            ast::TypeMember::Object(nested) => collect_object_decl(
                source,
                nested,
                &obj_fqn,
                lower,
                funs_by_fqn,
                ctors_by_type,
            )?,
        }
    }

    Ok(())
}

fn collect_ctor_decl(
    source: &SourceFile,
    type_fqn: &str,
    span: Span,
    params: &[ast::Param],
    lower: &mut TypeLowering<'_>,
    ctors_by_type: &mut HashMap<String, Vec<CtorDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    let params = lower_params(source, params, lower)?;
    ctors_by_type
        .entry(type_fqn.to_string())
        .or_default()
        .push(CtorDeclInfo {
            span,
            sig: CtorSigInfo { params },
        });
    Ok(())
}

fn collect_fun_decl(
    source: &SourceFile,
    fun: &ast::FunDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    funs_by_fqn: &mut HashMap<String, Vec<FunDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    let name = source.slice(fun.name.span).to_string();
    let fqn = join_prefix(prefix, &name);

    lower.push_type_params(&fun.type_params);
    let fun_eff_binding = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
            None => crate::ty::EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    let receiver = match &fun.receiver {
        Some(r) => Some(lower.lower_type_ref(r)?),
        None => None,
    };
    let params = lower_params(source, &fun.params, lower)?;
    let return_ty = match &fun.return_ty {
        Some(ret) => Some(lower.lower_type_ref(ret)?),
        None => None,
    };

    if fun_eff_binding {
        lower.pop_effect_row_param_binding();
    }
    lower.pop_type_params(&fun.type_params);

    funs_by_fqn.entry(fqn.clone()).or_default().push(FunDeclInfo {
        name_span: fun.name.span,
        sig: FunSigInfo {
            receiver,
            params,
            return_ty,
        },
    });

    Ok(())
}

fn lower_params(
    source: &SourceFile,
    params: &[ast::Param],
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<ParamInfo>, OverloadDeclError> {
    let mut out = Vec::with_capacity(params.len());
    for p in params {
        // `check_file_headers` 已要求参数必须带类型注解；这里作为健壮性兜底。
        let Some(ty) = &p.ty else {
            continue;
        };

        let id = lower.lower_type_ref(ty)?;
        let has_default = p.default_value.is_some();

        // 这里保留 name 主要是为后续 named args 冲突分析做铺垫；
        // 当前最小实现不将 name 纳入冲突判定。
        let _name = source.slice(p.name.span);

        out.push(ParamInfo { ty: id, has_default });
    }
    Ok(out)
}

fn check_fun_overload_set(fqn: &str, decls: &[FunDeclInfo]) -> Result<(), OverloadDeclError> {
    if decls.len() <= 1 {
        return Ok(());
    }

    // 以源码顺序稳定排序，便于错误稳定复现。
    let mut decls = decls.to_vec();
    decls.sort_by_key(|d| d.name_span.start);

    for i in 0..decls.len() {
        for j in (i + 1)..decls.len() {
            let a = &decls[i];
            let b = &decls[j];

            if call_sig_equal(&a.sig, &b.sig) {
                let reason = match (a.sig.return_ty, b.sig.return_ty) {
                    (Some(ra), Some(rb)) if ra != rb => {
                        "仅返回类型不同（返回类型不参与重载决议）".to_string()
                    }
                    _ => "重复或不可区分的签名".to_string(),
                };
                return Err(OverloadDeclError::Conflict {
                    fqn: fqn.to_string(),
                    reason,
                    conflict: b.name_span.into(),
                    previous: a.name_span.into(),
                });
            }

            if let Some(arity) = first_ambiguous_positional_arity(&a.sig, &b.sig) {
                let reason =
                    format!("默认参数导致在提供 {arity} 个实参时不可区分（位置调用）");
                return Err(OverloadDeclError::Conflict {
                    fqn: fqn.to_string(),
                    reason,
                    conflict: b.name_span.into(),
                    previous: a.name_span.into(),
                });
            }
        }
    }

    Ok(())
}

fn check_ctor_overload_set(type_fqn: &str, decls: &[CtorDeclInfo]) -> Result<(), OverloadDeclError> {
    if decls.len() <= 1 {
        return Ok(());
    }

    let mut decls = decls.to_vec();
    decls.sort_by_key(|d| d.span.start);

    for i in 0..decls.len() {
        for j in (i + 1)..decls.len() {
            let a = &decls[i];
            let b = &decls[j];

            if ctor_call_sig_equal(&a.sig, &b.sig) {
                return Err(OverloadDeclError::Conflict {
                    fqn: format!("{type_fqn}.<init>"),
                    reason: "重复或不可区分的构造器签名".to_string(),
                    conflict: b.span.into(),
                    previous: a.span.into(),
                });
            }

            if let Some(arity) = first_ambiguous_positional_ctor_arity(&a.sig, &b.sig) {
                let reason =
                    format!("默认参数导致在提供 {arity} 个实参时不可区分（位置调用）");
                return Err(OverloadDeclError::Conflict {
                    fqn: format!("{type_fqn}.<init>"),
                    reason,
                    conflict: b.span.into(),
                    previous: a.span.into(),
                });
            }
        }
    }

    Ok(())
}

fn call_sig_equal(a: &FunSigInfo, b: &FunSigInfo) -> bool {
    if a.receiver != b.receiver {
        return false;
    }
    if a.params.len() != b.params.len() {
        return false;
    }
    a.params
        .iter()
        .zip(&b.params)
        .all(|(pa, pb)| pa.ty == pb.ty)
}

fn ctor_call_sig_equal(a: &CtorSigInfo, b: &CtorSigInfo) -> bool {
    if a.params.len() != b.params.len() {
        return false;
    }
    a.params
        .iter()
        .zip(&b.params)
        .all(|(pa, pb)| pa.ty == pb.ty)
}

/// 返回一组 overload 在“位置调用 + 尾部默认参数省略”规则下的“第一个”会产生歧义的实参数量。
///
/// 说明：
/// - 这里的“歧义”指：存在某个 `k`，两者都接受 `k` 个位置实参，并且它们对这 `k` 个实参的期望类型完全一致；
/// - 当前实现刻意不考虑 subtyping/most-specific：这属于 overload resolution 的后续细化。
fn first_ambiguous_positional_arity(a: &FunSigInfo, b: &FunSigInfo) -> Option<usize> {
    if a.receiver != b.receiver {
        return None;
    }

    let min_a = min_positional_arity(&a.params);
    let min_b = min_positional_arity(&b.params);
    let max_k = a.params.len().min(b.params.len());

    for k in 0..=max_k {
        if k < min_a || k < min_b {
            continue;
        }
        if prefix_types_equal(&a.params, &b.params, k) {
            // k == len == len 的情况已经由 `call_sig_equal` 覆盖；这里避免重复报错。
            if k == a.params.len() && k == b.params.len() {
                continue;
            }
            return Some(k);
        }
    }
    None
}

fn first_ambiguous_positional_ctor_arity(a: &CtorSigInfo, b: &CtorSigInfo) -> Option<usize> {
    let min_a = min_positional_arity(&a.params);
    let min_b = min_positional_arity(&b.params);
    let max_k = a.params.len().min(b.params.len());

    for k in 0..=max_k {
        if k < min_a || k < min_b {
            continue;
        }
        if prefix_types_equal(&a.params, &b.params, k) {
            if k == a.params.len() && k == b.params.len() {
                continue;
            }
            return Some(k);
        }
    }
    None
}

fn prefix_types_equal(a: &[ParamInfo], b: &[ParamInfo], k: usize) -> bool {
    a.iter()
        .take(k)
        .zip(b.iter().take(k))
        .all(|(pa, pb)| pa.ty == pb.ty)
}

/// 位置调用下可省略的参数只来自“尾部默认参数”：
/// - `f(x:Int, y:Int=0)` 可用 `f(1)`
/// - `f(x:Int=0, y:Int)` 不能用 `f()`（需要 named args 支持才能省略中间参数）
fn min_positional_arity(params: &[ParamInfo]) -> usize {
    let mut trailing_defaults = 0usize;
    for p in params.iter().rev() {
        if p.has_default {
            trailing_defaults += 1;
        } else {
            break;
        }
    }
    params.len().saturating_sub(trailing_defaults)
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}
