//! interface 的最小语义检查（T0440 / spec §2.2.2）。
//!
//! 当前阶段目标：
//! - “实现列表”的基础合法性检查：
//!   - class/object：允许 1 个 class supertype ctor call + 多个 interface
//!   - struct/enum：只允许实现 interface（不允许 ctor call）
//!   - interface：只允许 extends interface（不允许 ctor call）
//! - 对 class/object/struct/enum 的 interface 实现做最小检查：
//!   - interface 的 **抽象方法**（无 body）必须被实现
//!   - interface 的 **默认方法**（有 body）不要求实现（暂不要求 codegen）
//!
//! 说明（刻意的简化）：
//! - 仅检查 direct supertype interface 的 direct members（不沿 interface 继承链向上追溯）；
//! - “签名一致性”当前只做最小匹配：同名 + receiver 有无一致 + 参数个数一致 + 方法 type params 数量一致；
//! - 默认方法冲突/diamond 等更复杂规则留给后续任务。

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::intrinsics::{NamedIntrinsicLoweringMode, named_intrinsic_audit_entry};
use crate::resolve::{FunOverload, ImportTable, Index};
use crate::source::SourceFile;
use crate::ty::{BuiltinTypes, TypeStore};

use super::builtin_annotations::BuiltinAnnotationFlags;
use super::lower::{TypeLowerError, TypeLowering};
use super::signature_match::{
    OwnerInstantiation, fun_decl_matches_overload_signature, fun_overloads_have_same_signature,
    nominal_from_type_id,
};
use super::{TypeEnv, TypeSymbolKind};

struct InterfaceCtx<'a, 'lower> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    env: &'a TypeEnv,
    lower: &'lower mut TypeLowering<'a>,
}

#[derive(Clone, Copy)]
struct NominalTarget<'a> {
    fqn: &'a str,
    nominal: &'a crate::ty::NominalType,
}

#[derive(Debug, Error, Diagnostic)]
pub enum InterfaceError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLower(#[from] TypeLowerError),

    #[error("`Continuation` 是 compiler-owned interface，用户代码不能实现或继承它")]
    #[diagnostic(code(scoop::typecheck::continuation_impl_not_allowed))]
    ContinuationImplNotAllowed {
        #[label("这里试图实现/继承 `Continuation`")]
        span: miette::SourceSpan,
    },

    #[error("实现/继承列表中的超类型必须是 interface：{found_fqn}")]
    #[diagnostic(code(scoop::typecheck::supertype_not_interface))]
    SupertypeNotInterface {
        found_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("超类型构造调用只允许 class：{found_fqn}")]
    #[diagnostic(code(scoop::typecheck::supertype_ctor_call_not_class))]
    SupertypeCtorCallNotClass {
        found_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("缺少 interface 成员实现：{type_fqn} 需要实现 {interface_fqn}.{member}")]
    #[diagnostic(code(scoop::typecheck::missing_interface_member))]
    MissingInterfaceMember {
        type_fqn: String,
        interface_fqn: String,
        member: String,
        #[label("这里声明实现了该 interface，但缺少对应成员实现")]
        span: miette::SourceSpan,
        #[label("该 interface 成员定义在这里")]
        member_span: miette::SourceSpan,
    },

    #[error("interface 成员实现存在歧义：{type_fqn} 对 {interface_fqn}.{member} 有多个候选实现")]
    #[diagnostic(code(scoop::typecheck::ambiguous_interface_member_impl))]
    AmbiguousInterfaceMemberImpl {
        type_fqn: String,
        interface_fqn: String,
        member: String,
        #[label("这里声明实现了该 interface，但对应成员实现不唯一")]
        span: miette::SourceSpan,
        #[label("该 interface 成员定义在这里")]
        member_span: miette::SourceSpan,
    },

    #[error(
        "`@Intrinsic` 类型的 interface override 必须是带 body 的普通 method：{type_fqn}.{member} 实现 {interface_fqn}.{member}"
    )]
    #[diagnostic(code(
        scoop::typecheck::intrinsic_type_interface_override_must_be_bodied_regular_method
    ))]
    IntrinsicTypeInterfaceOverrideMustBeBodiedRegularMethod {
        type_fqn: String,
        interface_fqn: String,
        member: String,
        #[label("这里必须提供普通 method body，不能使用 `@Intrinsic` / `@Extern` 或省略 body")]
        span: miette::SourceSpan,
        #[label("对应的 interface method 在这里")]
        interface_span: miette::SourceSpan,
    },
}

pub fn check_file_interfaces(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), InterfaceError> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
    let mut ctx = InterfaceCtx {
        source,
        file,
        index,
        env,
        lower: &mut lower,
    };
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    for item in &file.items {
        match item {
            ast::Item::Type(ty) => check_type_decl_interfaces(&mut ctx, ty, &pkg_prefix)?,
            ast::Item::Object(obj) => check_object_decl_interfaces(&mut ctx, obj, &pkg_prefix)?,
            ast::Item::TypeAlias(_)
            | ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_) => {}
        }
    }
    Ok(())
}

fn check_type_decl_interfaces(
    ctx: &mut InterfaceCtx<'_, '_>,
    decl: &ast::TypeDecl,
    prefix: &str,
) -> Result<(), InterfaceError> {
    let local_name = ctx.source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    ctx.lower.push_type_params(&decl.type_params);
    let ty_eff_binding_pushed = if let Some(eff_param) = &decl.eff_param {
        let name = ctx.source.slice(eff_param.name.span).to_string();
        ctx.lower
            .push_effect_row_param_marker_binding(name, eff_param.name.span);
        true
    } else {
        false
    };

    match decl.kind {
        ast::TypeKind::Class => {
            check_class_like_interfaces(ctx, &type_fqn, &decl.supertypes, decl.body.as_ref())?;
        }
        ast::TypeKind::Struct => {
            check_value_type_interfaces(ctx, &type_fqn, &decl.supertypes, decl.body.as_ref())?;
        }
        ast::TypeKind::Enum => {
            check_enum_decl_interfaces(ctx, &type_fqn, &decl.supertypes, decl.body.as_ref())?;
        }
        ast::TypeKind::Interface => {
            check_interface_decl_supertypes(
                ctx.source,
                ctx.file,
                &decl.supertypes,
                ctx.index,
                ctx.env,
            )?;
        }
        ast::TypeKind::Effect => {
            // 当前阶段不为 effect 引入额外 interface 语义（TODO T0602/T06xx）。
        }
    }

    if matches!(decl.kind, ast::TypeKind::Class | ast::TypeKind::Struct)
        && BuiltinAnnotationFlags::from_annotations(ctx.source, &decl.annotations).is_intrinsic
    {
        check_intrinsic_type_interface_impl_shape(
            ctx,
            &type_fqn,
            &decl.supertypes,
            decl.body.as_ref(),
        )?;
    }

    // 递归检查 nested types / nested objects。
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    check_type_decl_interfaces(ctx, nested, &type_fqn)?;
                }
                ast::TypeMember::Object(obj) => {
                    check_object_decl_interfaces(ctx, obj, &type_fqn)?;
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
        ctx.lower.pop_effect_row_param_binding();
    }
    ctx.lower.pop_type_params(&decl.type_params);

    Ok(())
}

fn check_object_decl_interfaces(
    ctx: &mut InterfaceCtx<'_, '_>,
    obj: &ast::ObjectDecl,
    prefix: &str,
) -> Result<(), InterfaceError> {
    // Kotlin-like：未命名 companion object 具有隐式名字 `Companion`（resolver/index 侧同样使用该名字）。
    let obj_name = obj
        .name
        .as_ref()
        .map(|id| ctx.source.slice(id.span).to_string())
        .unwrap_or_else(|| "Companion".to_string());

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    check_class_like_interfaces(ctx, &obj_fqn, &obj.supertypes, obj.body.as_ref())?;

    let Some(body) = &obj.body else {
        return Ok(());
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                check_type_decl_interfaces(ctx, nested, &obj_fqn)?;
            }
            ast::TypeMember::Object(nested) => {
                check_object_decl_interfaces(ctx, nested, &obj_fqn)?;
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }

    Ok(())
}

fn check_interface_decl_supertypes(
    source: &SourceFile,
    file: &ast::File,
    supertypes: &[ast::SuperType],
    index: &Index,
    env: &TypeEnv,
) -> Result<(), InterfaceError> {
    for st in supertypes {
        let Some(fqn) = index.type_ref_to_fqn_in_file(source, file, &st.ty) else {
            continue;
        };

        reject_compiler_owned_continuation_interface(&fqn, st.ty.span())?;

        if st.ctor_args_span.is_some() {
            return Err(InterfaceError::SupertypeCtorCallNotClass {
                found_fqn: fqn,
                span: st.ty.span().into(),
            });
        }

        if !is_interface(env, &fqn) {
            return Err(InterfaceError::SupertypeNotInterface {
                found_fqn: fqn,
                span: st.ty.span().into(),
            });
        }
    }
    Ok(())
}

fn check_class_like_interfaces(
    ctx: &mut InterfaceCtx<'_, '_>,
    type_fqn: &str,
    supertypes: &[ast::SuperType],
    body: Option<&ast::TypeBody>,
) -> Result<(), InterfaceError> {
    // 解析 super class（若有）；只用于“继承的成员也可用于满足 interface”的最小 fallback。
    // 注：更完整的继承链查找留给后续任务。
    let superclass = supertypes.iter().find(|st| st.ctor_args_span.is_some());
    let superclass_fqn = superclass.and_then(|st| {
        ctx.index
            .type_ref_to_fqn_in_file(ctx.source, ctx.file, &st.ty)
    });
    let superclass_nominal = match superclass {
        Some(st) => {
            let ty = ctx.lower.lower_type_ref(&st.ty)?;
            nominal_from_type_id(ty, ctx.lower)
        }
        None => None,
    };

    for st in supertypes {
        let Some(interface_fqn) = ctx
            .index
            .type_ref_to_fqn_in_file(ctx.source, ctx.file, &st.ty)
        else {
            continue;
        };

        reject_compiler_owned_continuation_interface(&interface_fqn, st.ty.span())?;

        if st.ctor_args_span.is_some() {
            // ctor call 只允许 class。
            if !matches!(
                nominal_kind(ctx.env, &interface_fqn),
                Some(ast::TypeKind::Class)
            ) {
                return Err(InterfaceError::SupertypeCtorCallNotClass {
                    found_fqn: interface_fqn,
                    span: st.ty.span().into(),
                });
            }
            continue;
        }

        if !is_interface(ctx.env, &interface_fqn) {
            return Err(InterfaceError::SupertypeNotInterface {
                found_fqn: interface_fqn,
                span: st.ty.span().into(),
            });
        }

        let interface_ty = ctx.lower.lower_type_ref(&st.ty)?;
        let Some(interface_nominal) = nominal_from_type_id(interface_ty, ctx.lower) else {
            continue;
        };
        let superclass = superclass_fqn
            .as_deref()
            .zip(superclass_nominal.as_ref())
            .map(|(fqn, nominal)| NominalTarget { fqn, nominal });
        check_one_interface_impl(
            ctx,
            type_fqn,
            body,
            superclass,
            NominalTarget {
                fqn: &interface_fqn,
                nominal: &interface_nominal,
            },
            st.ty.span(),
        )?;
    }

    Ok(())
}

fn check_value_type_interfaces(
    ctx: &mut InterfaceCtx<'_, '_>,
    type_fqn: &str,
    supertypes: &[ast::SuperType],
    body: Option<&ast::TypeBody>,
) -> Result<(), InterfaceError> {
    for st in supertypes {
        let Some(interface_fqn) = ctx
            .index
            .type_ref_to_fqn_in_file(ctx.source, ctx.file, &st.ty)
        else {
            continue;
        };

        reject_compiler_owned_continuation_interface(&interface_fqn, st.ty.span())?;

        if st.ctor_args_span.is_some() {
            return Err(InterfaceError::SupertypeCtorCallNotClass {
                found_fqn: interface_fqn,
                span: st.ty.span().into(),
            });
        }

        if !is_interface(ctx.env, &interface_fqn) {
            return Err(InterfaceError::SupertypeNotInterface {
                found_fqn: interface_fqn,
                span: st.ty.span().into(),
            });
        }

        let interface_ty = ctx.lower.lower_type_ref(&st.ty)?;
        let Some(interface_nominal) = nominal_from_type_id(interface_ty, ctx.lower) else {
            continue;
        };
        check_one_interface_impl(
            ctx,
            type_fqn,
            body,
            None,
            NominalTarget {
                fqn: &interface_fqn,
                nominal: &interface_nominal,
            },
            st.ty.span(),
        )?;
    }

    Ok(())
}

fn check_enum_decl_interfaces(
    ctx: &mut InterfaceCtx<'_, '_>,
    type_fqn: &str,
    supertypes: &[ast::SuperType],
    body: Option<&ast::TypeBody>,
) -> Result<(), InterfaceError> {
    // spec §2.3.2.1：value-only enum 的 `:` 后第一个类型是“底层整型表示”，并非 interface。
    //
    // 说明：
    // - 这里根据“第一个 supertype 是否为 interface”做最小消歧：
    //   - 非 interface：视为 value-only enum 的底层类型 → 跳过；
    //   - interface：视为普通 interface 实现列表 → 不跳过；
    // - 底层类型是否为整型标量由 `typecheck::check_file_type_refs`（TypeLowerError）负责门禁。
    let should_skip_first = supertypes.first().is_some_and(|st| {
        let Some(first_fqn) = ctx
            .index
            .type_ref_to_fqn_in_file(ctx.source, ctx.file, &st.ty)
        else {
            return false;
        };
        !is_interface(ctx.env, &first_fqn)
    });

    let interface_supertypes = if should_skip_first {
        &supertypes[1..]
    } else {
        supertypes
    };

    check_value_type_interfaces(ctx, type_fqn, interface_supertypes, body)
}

fn check_one_interface_impl(
    ctx: &mut InterfaceCtx<'_, '_>,
    type_fqn: &str,
    body: Option<&ast::TypeBody>,
    superclass: Option<NominalTarget<'_>>,
    interface: NominalTarget<'_>,
    interface_use_span: crate::span::Span,
) -> Result<(), InterfaceError> {
    for required in required_abstract_interface_funs(ctx.index, interface.fqn) {
        match member_fun_match_in_body(
            ctx.source,
            body,
            required,
            OwnerInstantiation {
                fqn: interface.fqn,
                nominal: interface.nominal,
            },
            ctx.lower,
            ctx.env,
        )? {
            MemberFunMatch::Unique => continue,
            MemberFunMatch::Ambiguous => {
                return Err(InterfaceError::AmbiguousInterfaceMemberImpl {
                    type_fqn: type_fqn.to_string(),
                    interface_fqn: interface.fqn.to_string(),
                    member: required.symbol.name.clone(),
                    span: interface_use_span.into(),
                    member_span: required.symbol.span.into(),
                });
            }
            MemberFunMatch::None => {}
        }

        if let Some(base) = superclass {
            match member_fun_match_in_overloads(
                ctx,
                base,
                required,
                OwnerInstantiation {
                    fqn: interface.fqn,
                    nominal: interface.nominal,
                },
            )? {
                MemberFunMatch::Unique => continue,
                MemberFunMatch::Ambiguous => {
                    return Err(InterfaceError::AmbiguousInterfaceMemberImpl {
                        type_fqn: type_fqn.to_string(),
                        interface_fqn: interface.fqn.to_string(),
                        member: required.symbol.name.clone(),
                        span: interface_use_span.into(),
                        member_span: required.symbol.span.into(),
                    });
                }
                MemberFunMatch::None => {}
            }
        }

        return Err(InterfaceError::MissingInterfaceMember {
            type_fqn: type_fqn.to_string(),
            interface_fqn: interface.fqn.to_string(),
            member: required.symbol.name.clone(),
            span: interface_use_span.into(),
            member_span: required.symbol.span.into(),
        });
    }

    Ok(())
}

fn check_intrinsic_type_interface_impl_shape(
    ctx: &mut InterfaceCtx<'_, '_>,
    type_fqn: &str,
    supertypes: &[ast::SuperType],
    body: Option<&ast::TypeBody>,
) -> Result<(), InterfaceError> {
    let Some(body) = body else {
        return Ok(());
    };
    let interface_funs = direct_interface_fun_targets(ctx, supertypes)?;
    if interface_funs.is_empty() {
        return Ok(());
    }

    for member in &body.members {
        let ast::TypeMember::Fun(fun) = member else {
            continue;
        };
        let mut matched = None;
        for (interface_fqn, interface_nominal, interface_fun) in &interface_funs {
            if ctx.source.slice(fun.name.span) != interface_fun.symbol.name {
                continue;
            }
            if fun_decl_matches_overload_signature(
                ctx.source,
                fun,
                interface_fun,
                OwnerInstantiation {
                    fqn: interface_fqn,
                    nominal: interface_nominal,
                },
                ctx.lower,
                ctx.env,
            )? {
                matched = Some((interface_fqn, interface_fun));
                break;
            }
        }
        let Some((interface_fqn, interface_fun)) = matched else {
            continue;
        };

        let flags = BuiltinAnnotationFlags::from_annotations(ctx.source, &fun.annotations);
        if named_ir_intrinsic_override_is_allowed(&flags)
            || (!flags.is_intrinsic
                && !flags.is_extern
                && !matches!(fun.body, ast::FunBody::Missing))
        {
            continue;
        }

        return Err(
            InterfaceError::IntrinsicTypeInterfaceOverrideMustBeBodiedRegularMethod {
                type_fqn: type_fqn.to_string(),
                interface_fqn: interface_fqn.clone(),
                member: ctx.source.slice(fun.name.span).to_string(),
                span: fun.name.span.into(),
                interface_span: interface_fun.symbol.span.into(),
            },
        );
    }

    Ok(())
}

fn named_ir_intrinsic_override_is_allowed(flags: &BuiltinAnnotationFlags) -> bool {
    let Some(entry_name) = flags.intrinsic_entry_name.as_deref() else {
        return false;
    };
    named_intrinsic_audit_entry(entry_name)
        .is_some_and(|entry| entry.lowering_mode == NamedIntrinsicLoweringMode::IrEmission)
}

fn direct_interface_fun_targets<'a, 'lower>(
    ctx: &mut InterfaceCtx<'a, 'lower>,
    supertypes: &[ast::SuperType],
) -> Result<Vec<(String, crate::ty::NominalType, &'a FunOverload)>, InterfaceError> {
    let mut out = Vec::new();
    for st in supertypes.iter().filter(|st| st.ctor_args_span.is_none()) {
        let Some(interface_fqn) = ctx
            .index
            .type_ref_to_fqn_in_file(ctx.source, ctx.file, &st.ty)
        else {
            continue;
        };
        if !is_interface(ctx.env, &interface_fqn) {
            continue;
        }
        let interface_ty = ctx.lower.lower_type_ref(&st.ty)?;
        let Some(interface_nominal) = nominal_from_type_id(interface_ty, ctx.lower) else {
            continue;
        };
        for overload in direct_interface_funs(ctx.index, &interface_fqn) {
            out.push((interface_fqn.clone(), interface_nominal.clone(), overload));
        }
    }
    Ok(out)
}

fn direct_interface_funs<'a>(index: &'a Index, interface_fqn: &str) -> Vec<&'a FunOverload> {
    let prefix = format!("{interface_fqn}.");
    let mut out = Vec::new();

    for (fqn, syms) in &index.by_fqn {
        if !fqn.starts_with(&prefix) {
            continue;
        }
        // 排除 nested type/object 的成员：我们只关心 `Interface.member`。
        let rest = &fqn[prefix.len()..];
        if rest.contains('.') {
            continue;
        }

        out.extend(syms.fun.iter());
    }

    out
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberFunMatch {
    None,
    Unique,
    Ambiguous,
}

fn member_fun_match_in_body(
    source: &SourceFile,
    body: Option<&ast::TypeBody>,
    required: &FunOverload,
    required_owner: OwnerInstantiation<'_>,
    lower: &mut TypeLowering<'_>,
    env: &TypeEnv,
) -> Result<MemberFunMatch, InterfaceError> {
    let Some(body) = body else {
        return Ok(MemberFunMatch::None);
    };

    let mut matching = 0usize;
    for member in &body.members {
        let ast::TypeMember::Fun(fun) = member else {
            continue;
        };
        if source.slice(fun.name.span) != required.symbol.name {
            continue;
        }
        if fun_decl_matches_overload_signature(source, fun, required, required_owner, lower, env)? {
            matching += 1;
        }
    }

    Ok(member_fun_match_from_count(matching))
}

fn member_fun_match_in_overloads(
    ctx: &mut InterfaceCtx<'_, '_>,
    target: NominalTarget<'_>,
    required: &FunOverload,
    required_owner: OwnerInstantiation<'_>,
) -> Result<MemberFunMatch, InterfaceError> {
    let member_fqn = format!("{}.{}", target.fqn, required.symbol.name);
    let Some(syms) = ctx.index.by_fqn.get(&member_fqn) else {
        return Ok(MemberFunMatch::None);
    };

    let type_owner = OwnerInstantiation {
        fqn: target.fqn,
        nominal: target.nominal,
    };
    let mut matching = 0usize;
    for cand in &syms.fun {
        if fun_overloads_have_same_signature(
            ctx.source,
            cand,
            type_owner,
            required,
            required_owner,
            ctx.lower,
            ctx.env,
        )? {
            matching += 1;
        }
    }

    Ok(member_fun_match_from_count(matching))
}

fn member_fun_match_from_count(matching: usize) -> MemberFunMatch {
    match matching {
        0 => MemberFunMatch::None,
        1 => MemberFunMatch::Unique,
        _ => MemberFunMatch::Ambiguous,
    }
}

fn nominal_kind(env: &TypeEnv, fqn: &str) -> Option<ast::TypeKind> {
    let sym = env.type_symbol(fqn)?;
    match sym.kind {
        TypeSymbolKind::Nominal(kind) => Some(kind),
        TypeSymbolKind::TypeAlias => None,
    }
}

fn is_interface(env: &TypeEnv, fqn: &str) -> bool {
    matches!(nominal_kind(env, fqn), Some(ast::TypeKind::Interface))
}

fn reject_compiler_owned_continuation_interface(
    interface_fqn: &str,
    span: crate::span::Span,
) -> Result<(), InterfaceError> {
    if interface_fqn == "scoop.core.Continuation" {
        return Err(InterfaceError::ContinuationImplNotAllowed { span: span.into() });
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ast;
    use crate::parser::parse_file;
    use crate::resolve::{self, FileHeaders, Index};
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::typecheck;

    fn setup_interface_check(
        source_text: &str,
    ) -> (SourceFile, ast::File, Index, FileHeaders, TypeEnv) {
        let session = Session::with_options(SessionOptions::new()).unwrap();
        let source = SourceFile::new_virtual("<continuation_interface>", source_text);
        let mut ast = parse_file(&source).expect("parse");

        typecheck::check_file_headers(&source, &ast).expect("headers");
        typecheck::check_file_struct_decls(&source, &ast).expect("struct decls");

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in session.sysroot().index_files() {
                unit.push((&file.source, &file.ast));
            }
            unit.push((&source, &ast));
            Index::build(&unit).expect("index")
        };
        let headers = resolve::check_file_headers(&source, &ast, &index).expect("resolve headers");
        resolve::check_file_bodies(&source, &mut ast, &index, &headers).expect("resolve bodies");

        let mut env = TypeEnv::from_sysroot(session.sysroot(), &index).expect("type env");
        env.extend_from_file(&source, &ast, &index)
            .expect("extend type env");
        (source, ast, index, headers, env)
    }

    #[test]
    fn continuation_typecheck_rejects_user_impl_of_compiler_owned_continuation() {
        let (source, ast, index, headers, env) = setup_interface_check(
            r#"
package fixtures.typecheck

import scoop.core.*

class Fake() : Continuation<Int, Unit, eff Pure> {
    fun resume(value: Int): Unit / Raise<RuntimeError> {
        return
    }
}
"#,
        );

        let mut types = crate::ty::TypeStore::new();
        let builtins = types.intern_builtins();
        let err = check_file_interfaces(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .expect_err("用户代码不应允许实现 Continuation");

        assert!(matches!(
            err,
            InterfaceError::ContinuationImplNotAllowed { .. }
        ));
    }
}
