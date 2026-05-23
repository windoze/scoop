//! override / interface impl 的 effect row（`/ R`）约束检查（T0609 / spec §5.9.1）。
//!
//! 规则：
//! - 当一个方法 override 另一个方法（或实现 interface 抽象方法）时，
//!   overriding 的 effect row `R_over` 必须满足 `R_over ⊆ R_base`。
//! - `R_base` 的计算需要先对 receiver 的 use-site type args 与 use-site effect row args 做 substitution。
//!   例如：`Disposable<eff E>.dispose(): Unit / E` 在 `Disposable<eff IO>` 上实例化后其 `R_base = IO`。
//!
//! 说明（当前阶段的刻意简化）：
//! - 只检查 direct superclass（不沿继承链向上搜索）；
//! - 对 interface 仅检查 direct supertypes 的 direct abstract members（不沿 interface 继承链向上追溯）；
//! - 对 member fun 的“override/匹配”仍沿用既有最小判定：同名 + receiver 有无一致 + 参数个数一致 + 方法 type params 数量一致；
//! - 对 method-level `<eff E = ...>` 参数：按现有 lowering 语义用默认值绑定进行 substitution（更强的符号约束求解后置）。

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{FunOverload, ImportTable, Index};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::lower::{TypeLowerError, TypeLowering};
use super::{TypeEnv, TypeSymbolKind};

#[derive(Debug, Error, Diagnostic)]
pub enum OverrideEffectError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error(
        "override 方法的 effect row 不能增加：{class_fqn}.{member} 声明为 {over_row}，但基类 {base_fqn}.{member} 允许 {base_row}"
    )]
    #[diagnostic(code(scoop::typecheck::override_effect_row_not_contained))]
    ClassOverrideEffectRowNotContained {
        class_fqn: String,
        base_fqn: String,
        member: String,
        over_row: String,
        base_row: String,
        #[label("override 的 effect row 在这里")]
        span: miette::SourceSpan,
        #[label("被覆盖的方法定义在这里")]
        base_span: miette::SourceSpan,
    },

    #[error(
        "实现 interface 方法的 effect row 不能增加：{type_fqn}.{member} 声明为 {impl_row}，但 interface {interface_fqn}.{member} 允许 {base_row}"
    )]
    #[diagnostic(code(scoop::typecheck::override_effect_row_not_contained))]
    InterfaceImplEffectRowNotContained {
        type_fqn: String,
        interface_fqn: String,
        member: String,
        impl_row: String,
        base_row: String,
        #[label("实现的方法在这里")]
        span: miette::SourceSpan,
        #[label("interface 签名在这里")]
        base_span: miette::SourceSpan,
    },
}

type OverrideEffectResult<T> = Result<T, Box<OverrideEffectError>>;

fn override_effect_err(error: OverrideEffectError) -> Box<OverrideEffectError> {
    Box::new(error)
}

impl From<TypeLowerError> for Box<OverrideEffectError> {
    fn from(error: TypeLowerError) -> Self {
        override_effect_err(OverrideEffectError::from(error))
    }
}

#[derive(Clone, Copy)]
struct TypeInterfaceImplTarget<'a> {
    type_fqn: &'a str,
    supertypes: &'a [ast::SuperType],
    body: Option<&'a ast::TypeBody>,
}

pub fn check_file_override_effects(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> OverrideEffectResult<()> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);

    let pkg_prefix = package_prefix(source, file.package.as_ref());
    for item in &file.items {
        match item {
            ast::Item::Type(ty) => check_type_decl_override_effects(
                source,
                file,
                ty,
                &pkg_prefix,
                index,
                env,
                &mut lower,
            )?,
            ast::Item::Object(obj) => check_object_decl_override_effects(
                source,
                file,
                obj,
                &pkg_prefix,
                index,
                env,
                &mut lower,
            )?,
            ast::Item::TypeAlias(_)
            | ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_) => {}
        }
    }

    Ok(())
}

fn check_type_decl_override_effects(
    source: &SourceFile,
    file: &ast::File,
    decl: &ast::TypeDecl,
    prefix: &str,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
) -> OverrideEffectResult<()> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    lower.push_type_params(&decl.type_params);
    let ty_eff_binding_pushed = if let Some(eff_param) = &decl.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
            None => EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    match decl.kind {
        ast::TypeKind::Class => {
            check_class_member_override_effects(source, file, decl, &type_fqn, index, env, lower)?;
            check_type_interface_impl_effects(
                source,
                file,
                index,
                env,
                lower,
                TypeInterfaceImplTarget {
                    type_fqn: &type_fqn,
                    supertypes: &decl.supertypes,
                    body: decl.body.as_ref(),
                },
            )?;
        }
        ast::TypeKind::Struct | ast::TypeKind::Enum => {
            check_type_interface_impl_effects(
                source,
                file,
                index,
                env,
                lower,
                TypeInterfaceImplTarget {
                    type_fqn: &type_fqn,
                    supertypes: &decl.supertypes,
                    body: decl.body.as_ref(),
                },
            )?;
        }
        ast::TypeKind::Interface | ast::TypeKind::Effect => {
            // 当前阶段不对 interface/effect 声明引入额外 override 语义检查。
        }
    }

    // 递归检查 nested types / nested objects。
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    check_type_decl_override_effects(
                        source, file, nested, &type_fqn, index, env, lower,
                    )?;
                }
                ast::TypeMember::Object(obj) => {
                    check_object_decl_override_effects(
                        source, file, obj, &type_fqn, index, env, lower,
                    )?;
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }

    if ty_eff_binding_pushed {
        lower.pop_effect_row_param_binding();
    }
    lower.pop_type_params(&decl.type_params);

    Ok(())
}

fn check_object_decl_override_effects(
    source: &SourceFile,
    file: &ast::File,
    obj: &ast::ObjectDecl,
    prefix: &str,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
) -> OverrideEffectResult<()> {
    // Kotlin-like：未命名 companion object 具有隐式名字 `Companion`（resolver/index 侧同样使用该名字）。
    let obj_name = obj
        .name
        .as_ref()
        .map(|id| source.slice(id.span).to_string())
        .unwrap_or_else(|| "Companion".to_string());

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    check_object_member_override_effects(source, file, obj, &obj_fqn, index, env, lower)?;
    check_type_interface_impl_effects(
        source,
        file,
        index,
        env,
        lower,
        TypeInterfaceImplTarget {
            type_fqn: &obj_fqn,
            supertypes: &obj.supertypes,
            body: obj.body.as_ref(),
        },
    )?;

    // 递归检查 nested types / nested objects。
    if let Some(body) = &obj.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    check_type_decl_override_effects(
                        source, file, nested, &obj_fqn, index, env, lower,
                    )?;
                }
                ast::TypeMember::Object(nested) => {
                    check_object_decl_override_effects(
                        source, file, nested, &obj_fqn, index, env, lower,
                    )?;
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }

    Ok(())
}

fn check_class_member_override_effects(
    source: &SourceFile,
    file: &ast::File,
    decl: &ast::TypeDecl,
    class_fqn: &str,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
) -> OverrideEffectResult<()> {
    let Some(body) = &decl.body else {
        return Ok(());
    };

    let superclass = decl
        .supertypes
        .iter()
        .find(|st| st.ctor_args_span.is_some());
    let Some(superclass) = superclass else {
        return Ok(());
    };
    let Some(base_fqn) = index.type_ref_to_fqn_in_file(source, file, &superclass.ty) else {
        return Ok(());
    };

    // 解析 base class 的 use-site instantiation（type args + eff row args）。
    let base_ty = lower.lower_type_ref(&superclass.ty)?;
    let Some(base_nominal) = nominal_from_type_id(base_ty, lower) else {
        return Ok(());
    };

    for member in &body.members {
        let ast::TypeMember::Fun(fun) = member else {
            continue;
        };
        if !fun.modifiers.contains(&ast::Modifier::Override) {
            continue;
        }

        let name = source.slice(fun.name.span).to_string();
        let over_row = lower_fun_decl_effect_row(source, fun, lower)?;

        let base_member_fqn = format!("{base_fqn}.{name}");
        let base_overloads = index
            .by_fqn
            .get(&base_member_fqn)
            .map(|syms| syms.fun.as_slice())
            .unwrap_or(&[]);

        let derived_param_len = fun.params.len();
        let derived_has_receiver = fun.receiver.is_some();
        let derived_type_params_len = fun.type_params.len();

        let matching = base_overloads
            .iter()
            .filter(|o| {
                o.sig.params.len() == derived_param_len
                    && o.sig.receiver.is_some() == derived_has_receiver
                    && o.sig.type_params.len() == derived_type_params_len
                    && o.symbol.modifiers.is_overridable()
            })
            .collect::<Vec<_>>();

        if matching.is_empty() {
            continue;
        }

        let mut ok = false;
        let mut first_base_row: Option<EffectRow> = None;
        let mut first_base_span: Option<miette::SourceSpan> = None;

        for cand in matching.iter().copied() {
            let base_row = lower_fun_overload_effect_row_with_receiver_instantiation(
                source,
                lower,
                env,
                &base_fqn,
                &base_nominal,
                cand,
            )?;
            if first_base_row.is_none() {
                first_base_row = Some(base_row.clone());
                first_base_span = Some(cand.symbol.span.into());
            }
            if over_row.is_subset_of(&base_row) {
                ok = true;
                break;
            }
        }

        if ok {
            continue;
        }

        let base_row = first_base_row.unwrap_or_else(EffectRow::pure);
        let over_row_s = fmt_effect_row(&over_row, lower);
        let base_row_s = fmt_effect_row(&base_row, lower);
        let span = fun
            .effects
            .as_ref()
            .map(|e| e.span)
            .unwrap_or(fun.name.span);

        return Err(override_effect_err(
            OverrideEffectError::ClassOverrideEffectRowNotContained {
                class_fqn: class_fqn.to_string(),
                base_fqn: base_fqn.to_string(),
                member: name,
                over_row: over_row_s,
                base_row: base_row_s,
                span: span.into(),
                base_span: first_base_span.unwrap_or_else(|| superclass.ty.span().into()),
            },
        ));
    }

    Ok(())
}

fn check_object_member_override_effects(
    source: &SourceFile,
    file: &ast::File,
    obj: &ast::ObjectDecl,
    obj_fqn: &str,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
) -> OverrideEffectResult<()> {
    let Some(body) = &obj.body else {
        return Ok(());
    };

    let superclass = obj.supertypes.iter().find(|st| st.ctor_args_span.is_some());
    let Some(superclass) = superclass else {
        return Ok(());
    };
    let Some(base_fqn) = index.type_ref_to_fqn_in_file(source, file, &superclass.ty) else {
        return Ok(());
    };

    let base_ty = lower.lower_type_ref(&superclass.ty)?;
    let Some(base_nominal) = nominal_from_type_id(base_ty, lower) else {
        return Ok(());
    };

    for member in &body.members {
        let ast::TypeMember::Fun(fun) = member else {
            continue;
        };
        if !fun.modifiers.contains(&ast::Modifier::Override) {
            continue;
        }

        let name = source.slice(fun.name.span).to_string();
        let over_row = lower_fun_decl_effect_row(source, fun, lower)?;

        let base_member_fqn = format!("{base_fqn}.{name}");
        let base_overloads = index
            .by_fqn
            .get(&base_member_fqn)
            .map(|syms| syms.fun.as_slice())
            .unwrap_or(&[]);

        let derived_param_len = fun.params.len();
        let derived_has_receiver = fun.receiver.is_some();
        let derived_type_params_len = fun.type_params.len();

        let matching = base_overloads
            .iter()
            .filter(|o| {
                o.sig.params.len() == derived_param_len
                    && o.sig.receiver.is_some() == derived_has_receiver
                    && o.sig.type_params.len() == derived_type_params_len
                    && o.symbol.modifiers.is_overridable()
            })
            .collect::<Vec<_>>();

        if matching.is_empty() {
            continue;
        }

        let mut ok = false;
        let mut first_base_row: Option<EffectRow> = None;
        let mut first_base_span: Option<miette::SourceSpan> = None;

        for cand in matching.iter().copied() {
            let base_row = lower_fun_overload_effect_row_with_receiver_instantiation(
                source,
                lower,
                env,
                &base_fqn,
                &base_nominal,
                cand,
            )?;
            if first_base_row.is_none() {
                first_base_row = Some(base_row.clone());
                first_base_span = Some(cand.symbol.span.into());
            }
            if over_row.is_subset_of(&base_row) {
                ok = true;
                break;
            }
        }

        if ok {
            continue;
        }

        let base_row = first_base_row.unwrap_or_else(EffectRow::pure);
        let over_row_s = fmt_effect_row(&over_row, lower);
        let base_row_s = fmt_effect_row(&base_row, lower);
        let span = fun
            .effects
            .as_ref()
            .map(|e| e.span)
            .unwrap_or(fun.name.span);

        return Err(override_effect_err(
            OverrideEffectError::ClassOverrideEffectRowNotContained {
                class_fqn: obj_fqn.to_string(),
                base_fqn: base_fqn.to_string(),
                member: name,
                over_row: over_row_s,
                base_row: base_row_s,
                span: span.into(),
                base_span: first_base_span.unwrap_or_else(|| superclass.ty.span().into()),
            },
        ));
    }

    Ok(())
}

fn check_type_interface_impl_effects(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    target: TypeInterfaceImplTarget<'_>,
) -> OverrideEffectResult<()> {
    let TypeInterfaceImplTarget {
        type_fqn,
        supertypes,
        body,
    } = target;
    // 计算 direct superclass（若有），用于“继承的成员也可用于满足 interface”的最小 fallback（与 T0440 保持一致）。
    let superclass = supertypes.iter().find(|st| st.ctor_args_span.is_some());
    let superclass_fqn =
        superclass.and_then(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty));
    let superclass_nominal = if let Some(st) = superclass {
        let base_ty = lower.lower_type_ref(&st.ty)?;
        nominal_from_type_id(base_ty, lower)
    } else {
        None
    };

    for st in supertypes {
        if st.ctor_args_span.is_some() {
            continue;
        }
        let Some(interface_fqn) = index.type_ref_to_fqn_in_file(source, file, &st.ty) else {
            continue;
        };

        if !is_interface(env, &interface_fqn) {
            continue;
        }

        let interface_ty = lower.lower_type_ref(&st.ty)?;
        let Some(interface_nominal) = nominal_from_type_id(interface_ty, lower) else {
            continue;
        };

        // 对 interface 的 direct abstract methods 做最小 override effect row 检查。
        for required in required_abstract_interface_funs(index, &interface_fqn) {
            let member_name = required.symbol.name.as_str();

            // 先尝试在当前类型体内找到匹配实现（用 AST 的 span 更精确）。
            let mut impl_row: Option<(EffectRow, Span)> = None;
            if let Some(body) = body {
                for m in &body.members {
                    let ast::TypeMember::Fun(fun) = m else {
                        continue;
                    };
                    let name = source.slice(fun.name.span);
                    if name != member_name {
                        continue;
                    }
                    if fun.params.len() != required.sig.params.len() {
                        continue;
                    }
                    if fun.receiver.is_some() != required.sig.receiver.is_some() {
                        continue;
                    }
                    if fun.type_params.len() != required.sig.type_params.len() {
                        continue;
                    }

                    let row = lower_fun_decl_effect_row(source, fun, lower)?;
                    let span = fun
                        .effects
                        .as_ref()
                        .map(|e| e.span)
                        .unwrap_or(fun.name.span);
                    impl_row = Some((row, span));
                    break;
                }
            }

            // 若当前类型未声明实现，允许由 direct superclass 提供（与 T0440 行为一致）。
            if impl_row.is_none() {
                let (Some(base_fqn), Some(base_nominal)) =
                    (superclass_fqn.as_deref(), superclass_nominal.as_ref())
                else {
                    continue;
                };

                let member_fqn = format!("{base_fqn}.{member_name}");
                let base_overloads = index
                    .by_fqn
                    .get(&member_fqn)
                    .map(|syms| syms.fun.as_slice())
                    .unwrap_or(&[]);

                let matching = base_overloads.iter().find(|cand| {
                    cand.sig.params.len() == required.sig.params.len()
                        && cand.sig.receiver.is_some() == required.sig.receiver.is_some()
                        && cand.sig.type_params.len() == required.sig.type_params.len()
                });

                let Some(cand) = matching else {
                    continue;
                };

                let row = lower_fun_overload_effect_row_with_receiver_instantiation(
                    source,
                    lower,
                    env,
                    base_fqn,
                    base_nominal,
                    cand,
                )?;
                impl_row = Some((row, cand.symbol.span));
            }

            let Some((impl_row, impl_span)) = impl_row else {
                continue;
            };

            let base_row = lower_interface_fun_overload_effect_row_with_receiver_instantiation(
                source,
                lower,
                env,
                &interface_fqn,
                &interface_nominal,
                required,
            )?;

            if impl_row.is_subset_of(&base_row) {
                continue;
            }

            return Err(override_effect_err(
                OverrideEffectError::InterfaceImplEffectRowNotContained {
                    type_fqn: type_fqn.to_string(),
                    interface_fqn: interface_fqn.to_string(),
                    member: member_name.to_string(),
                    impl_row: fmt_effect_row(&impl_row, lower),
                    base_row: fmt_effect_row(&base_row, lower),
                    span: impl_span.into(),
                    base_span: required.symbol.span.into(),
                },
            ));
        }
    }

    Ok(())
}

fn lower_fun_decl_effect_row(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
) -> OverrideEffectResult<EffectRow> {
    lower.push_type_params(&fun.type_params);
    let eff_binding_pushed = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
            None => EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    let row = lower.lower_effect_row_expr(fun.effects.as_ref())?;

    if eff_binding_pushed {
        lower.pop_effect_row_param_binding();
    }
    lower.pop_type_params(&fun.type_params);
    Ok(row)
}

fn lower_fun_overload_effect_row_with_receiver_instantiation(
    fallback_source: &SourceFile,
    lower: &mut TypeLowering<'_>,
    env: &TypeEnv,
    receiver_fqn: &str,
    receiver_nominal: &crate::ty::NominalType,
    fun: &FunOverload,
) -> OverrideEffectResult<EffectRow> {
    let Some(receiver_sym) = env.type_symbol(receiver_fqn) else {
        return Ok(EffectRow::pure());
    };

    let type_bindings = receiver_sym
        .type_param_names
        .iter()
        .cloned()
        .zip(receiver_nominal.args.iter().copied())
        .collect::<Vec<_>>();

    let mut eff_bindings: Vec<(String, EffectRow)> = Vec::new();
    if let Some(eff_param) = &receiver_sym.eff_param {
        let eff_row = receiver_nominal.eff.clone().unwrap_or_else(EffectRow::pure);
        eff_bindings.push((eff_param.name.clone(), eff_row));
    }

    // method-level `<eff E = ...>`：按现有语义用默认值进行 substitution。
    if let Some(eff_param) = &fun.sig.eff_param {
        let decl_source = env.source(&fun.symbol.decl_file).unwrap_or(fallback_source);
        let name = decl_source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr_in_decl_file_with_scopes(
                &fun.symbol.decl_file,
                type_bindings.iter().cloned(),
                eff_bindings.iter().cloned(),
                Some(expr),
            )?,
            None => EffectRow::pure(),
        };
        eff_bindings.push((name, default));
    }

    Ok(lower.lower_effect_row_expr_in_decl_file_with_scopes(
        &fun.symbol.decl_file,
        type_bindings,
        eff_bindings,
        fun.sig.effects.as_ref(),
    )?)
}

fn lower_interface_fun_overload_effect_row_with_receiver_instantiation(
    fallback_source: &SourceFile,
    lower: &mut TypeLowering<'_>,
    env: &TypeEnv,
    interface_fqn: &str,
    interface_nominal: &crate::ty::NominalType,
    fun: &FunOverload,
) -> OverrideEffectResult<EffectRow> {
    lower_fun_overload_effect_row_with_receiver_instantiation(
        fallback_source,
        lower,
        env,
        interface_fqn,
        interface_nominal,
        fun,
    )
}

fn required_abstract_interface_funs<'a>(
    index: &'a Index,
    interface_fqn: &str,
) -> Vec<&'a FunOverload> {
    let prefix = format!("{interface_fqn}.");
    let mut out = Vec::new();

    for (fqn, syms) in &index.by_fqn {
        if !fqn.starts_with(&prefix) {
            continue;
        }
        // 排除 nested type/object 的成员：我们只关心 `Interface.member`，而不是 `Interface.Nested.member`。
        let rest = &fqn[prefix.len()..];
        if rest.contains('.') {
            continue;
        }

        for o in &syms.fun {
            if o.has_body {
                continue;
            }
            out.push(o);
        }
    }

    out
}

fn nominal_from_type_id(ty: TypeId, lower: &TypeLowering<'_>) -> Option<crate::ty::NominalType> {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => Some(nominal),
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal),
        _ => None,
    }
}

fn fmt_effect_row(row: &EffectRow, lower: &TypeLowering<'_>) -> String {
    if row.terms.is_empty() {
        return "Pure".to_string();
    }
    row.terms
        .iter()
        .copied()
        .map(|e| lower.fmt_type(e))
        .collect::<Vec<_>>()
        .join(" + ")
}

fn is_interface(env: &TypeEnv, fqn: &str) -> bool {
    env.type_symbol(fqn)
        .is_some_and(|sym| matches!(sym.kind, TypeSymbolKind::Nominal(ast::TypeKind::Interface)))
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
