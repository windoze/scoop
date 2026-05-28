//! class 继承与 override 的最小语义检查（T0439 / Appendix B.2）。
//!
//! 当前阶段覆盖：
//! - class 单继承（只允许一个“基类构造调用”）；
//! - 继承 final class 的错误（class 默认 final，必须 `open`/`abstract`/`sealed` 才能被继承）；
//! - `sealed` class 仅允许在同一编译单元（同一源文件）内被直接继承（Kotlin-like）；
//! - override 必须显式声明 `override`；
//! - 只能 override `open` 或 `abstract` 成员。
//!
//! 说明：
//! - 当前只检查 **direct superclass**（不沿继承链向上查找）；
//! - 对 member fun 仅做“按参数个数匹配”的最小签名匹配，以避免把重载误判为 override；
//! - vtable/codegen 语义留给后续阶段。

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{ImportTable, Index};
use crate::source::SourceFile;
use crate::ty::{BuiltinTypes, TypeStore};

use super::TypeEnv;
use super::lower::{TypeLowerError, TypeLowering};
use super::signature_match::{
    OwnerInstantiation, fun_decl_matches_overload_signature, nominal_from_type_id,
};

struct InheritanceCtx<'a, 'lower> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    env: &'a TypeEnv,
    lower: &'lower mut TypeLowering<'a>,
}

#[derive(Clone, Copy)]
struct SuperclassTarget<'a> {
    fqn: &'a str,
    nominal: &'a crate::ty::NominalType,
}

#[derive(Debug, Error, Diagnostic)]
pub enum InheritanceError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLower(#[from] TypeLowerError),

    #[error("class 只支持单继承：{class_fqn}")]
    #[diagnostic(code(scoop::typecheck::multiple_superclasses))]
    MultipleSuperclasses {
        class_fqn: String,
        #[label("第一个基类在这里")]
        first: miette::SourceSpan,
        #[label("第二个基类在这里")]
        second: miette::SourceSpan,
    },

    #[error("不能继承 final class：{base_fqn}（需要 `open` / `abstract` / `sealed`）")]
    #[diagnostic(code(scoop::typecheck::superclass_not_open))]
    SuperclassNotOpen {
        base_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
        #[label("基类定义在这里")]
        base_span: miette::SourceSpan,
    },

    #[error("sealed class 只能在同一编译单元内被直接继承：{base_fqn}")]
    #[diagnostic(code(scoop::typecheck::sealed_inheritance_outside_compilation_unit))]
    SealedInheritanceOutsideCompilationUnit {
        base_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
        #[label("sealed class 定义在这里")]
        base_span: miette::SourceSpan,
    },

    #[error("必须显式声明 `override`：{class_fqn}.{member}")]
    #[diagnostic(code(scoop::typecheck::missing_override))]
    MissingOverride {
        class_fqn: String,
        member: String,
        #[label("这里缺少 `override`")]
        span: miette::SourceSpan,
        #[label("被覆盖的成员定义在这里")]
        base_span: miette::SourceSpan,
    },

    #[error("`override` 目标不存在：{class_fqn}.{member}")]
    #[diagnostic(code(scoop::typecheck::override_target_not_found))]
    OverrideTargetNotFound {
        class_fqn: String,
        member: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("不能 override 非 `open`/`abstract` 成员：{base_fqn}.{member}")]
    #[diagnostic(code(scoop::typecheck::cannot_override_final_member))]
    CannotOverrideFinalMember {
        base_fqn: String,
        member: String,
        #[label("这里")]
        span: miette::SourceSpan,
        #[label("该成员定义在这里")]
        base_span: miette::SourceSpan,
    },

    #[error("父类方法不是 `open` / `abstract`，不能被重写：{base_fqn}.{member}")]
    #[diagnostic(code(scoop::typecheck::override_non_open_method))]
    OverrideNonOpenMethod {
        base_fqn: String,
        member: String,
        #[label("这里声明了同签名方法")]
        span: miette::SourceSpan,
        #[label("非 open 父类方法定义在这里")]
        base_span: miette::SourceSpan,
    },

    #[error("virtual methods cannot have method-level type parameters: {type_fqn}.{member}")]
    #[diagnostic(
        code(scoop::typecheck::virtual_method_cannot_be_generic),
        help(
            "use a class-level type parameter, or convert this to a non-virtual / free-standing function"
        )
    )]
    VirtualMethodCannotBeGeneric {
        type_fqn: String,
        member: String,
        #[label("method-level type parameters are not allowed on virtual methods")]
        span: miette::SourceSpan,
    },
}

pub fn check_file_inheritance(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), InheritanceError> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
    let mut ctx = InheritanceCtx {
        source,
        file,
        index,
        env,
        lower: &mut lower,
    };
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    for item in &file.items {
        let ast::Item::Type(ty) = item else {
            continue;
        };
        check_type_decl_inheritance(&mut ctx, ty, &pkg_prefix)?;
    }
    Ok(())
}

fn check_type_decl_inheritance(
    ctx: &mut InheritanceCtx<'_, '_>,
    decl: &ast::TypeDecl,
    prefix: &str,
) -> Result<(), InheritanceError> {
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

    check_virtual_generic_methods(ctx.source, decl, &type_fqn)?;

    if matches!(decl.kind, ast::TypeKind::Class) {
        check_one_class(ctx, decl, &type_fqn)?;
    }

    // 递归检查 nested types（可能存在 nested class）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            let ast::TypeMember::Type(nested) = member else {
                continue;
            };
            check_type_decl_inheritance(ctx, nested, &type_fqn)?;
        }
    }

    if ty_eff_binding_pushed {
        ctx.lower.pop_effect_row_param_binding();
    }
    ctx.lower.pop_type_params(&decl.type_params);

    Ok(())
}

fn check_one_class(
    ctx: &mut InheritanceCtx<'_, '_>,
    decl: &ast::TypeDecl,
    class_fqn: &str,
) -> Result<(), InheritanceError> {
    let supers = decl
        .supertypes
        .iter()
        .filter(|st| st.ctor_args_span.is_some())
        .collect::<Vec<_>>();

    if supers.len() > 1 {
        return Err(InheritanceError::MultipleSuperclasses {
            class_fqn: class_fqn.to_string(),
            first: supers[0].span.into(),
            second: supers[1].span.into(),
        });
    }

    let superclass = supers.first().copied();
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

    if let (Some(st), Some(base_fqn)) = (superclass, superclass_fqn.as_deref()) {
        let Some(base_syms) = ctx.index.by_fqn.get(base_fqn) else {
            return Ok(());
        };
        let Some(base_type) = base_syms.ty.as_ref() else {
            return Ok(());
        };

        // B.2.1：class 默认 final；只有 open/abstract/sealed 才允许被继承。
        if !base_type.modifiers.is_inheritable() {
            return Err(InheritanceError::SuperclassNotOpen {
                base_fqn: base_fqn.to_string(),
                span: st.ty.span().into(),
                base_span: base_type.span.into(),
            });
        }

        // B.2.1：sealed class 仅允许在同一编译单元（同一源文件）内直接继承。
        if base_type.modifiers.sealed && base_type.decl_file != ctx.source.path() {
            return Err(InheritanceError::SealedInheritanceOutsideCompilationUnit {
                base_fqn: base_fqn.to_string(),
                span: st.ty.span().into(),
                base_span: base_type.span.into(),
            });
        }
    }

    // 若没有 superclass，则任何 `override` 都应报错。
    let Some(base_fqn) = superclass_fqn else {
        check_class_members_against_no_superclass(ctx, decl, class_fqn)?;
        return Ok(());
    };

    let Some(base_nominal) = superclass_nominal.as_ref() else {
        return Ok(());
    };
    check_class_members_against_superclass(
        ctx,
        decl,
        class_fqn,
        SuperclassTarget {
            fqn: &base_fqn,
            nominal: base_nominal,
        },
    )?;
    Ok(())
}

fn check_class_members_against_no_superclass(
    ctx: &mut InheritanceCtx<'_, '_>,
    decl: &ast::TypeDecl,
    class_fqn: &str,
) -> Result<(), InheritanceError> {
    let Some(body) = &decl.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(p) => {
                if !p.modifiers.contains(&ast::Modifier::Override) {
                    continue;
                }
                if direct_interface_property_override_target_exists(
                    ctx.source, ctx.file, decl, ctx.index, p,
                ) {
                    continue;
                }
                let name = ctx.source.slice(p.name.span).to_string();
                return Err(InheritanceError::OverrideTargetNotFound {
                    class_fqn: class_fqn.to_string(),
                    member: name,
                    span: p.name.span.into(),
                });
            }
            ast::TypeMember::Fun(f) => {
                if !f.modifiers.contains(&ast::Modifier::Override) {
                    continue;
                }
                if direct_interface_fun_override_target_exists(ctx, decl, f)? {
                    continue;
                }
                let name = ctx.source.slice(f.name.span).to_string();
                return Err(InheritanceError::OverrideTargetNotFound {
                    class_fqn: class_fqn.to_string(),
                    member: name,
                    span: f.name.span.into(),
                });
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Type(_)
            | ast::TypeMember::Object(_) => {}
        }
    }

    Ok(())
}

fn check_class_members_against_superclass(
    ctx: &mut InheritanceCtx<'_, '_>,
    decl: &ast::TypeDecl,
    class_fqn: &str,
    base: SuperclassTarget<'_>,
) -> Result<(), InheritanceError> {
    let Some(body) = &decl.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(p) => {
                match check_property_override(ctx.source, class_fqn, base.fqn, p, ctx.index) {
                    Err(InheritanceError::OverrideTargetNotFound { .. })
                        if direct_interface_property_override_target_exists(
                            ctx.source, ctx.file, decl, ctx.index, p,
                        ) => {}
                    other => other?,
                }
            }
            ast::TypeMember::Fun(f) => match check_fun_override(ctx, class_fqn, base, f) {
                Err(InheritanceError::OverrideTargetNotFound { .. })
                    if direct_interface_fun_override_target_exists(ctx, decl, f)? => {}
                other => other?,
            },
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Type(_)
            | ast::TypeMember::Object(_) => {}
        }
    }

    Ok(())
}

fn check_property_override(
    source: &SourceFile,
    class_fqn: &str,
    base_fqn: &str,
    p: &ast::PropertyDecl,
    index: &Index,
) -> Result<(), InheritanceError> {
    let name = source.slice(p.name.span).to_string();
    let wants_override = p.modifiers.contains(&ast::Modifier::Override);
    let base_member_fqn = format!("{base_fqn}.{name}");
    let base_member = index
        .by_fqn
        .get(&base_member_fqn)
        .and_then(|syms| syms.value.as_ref());

    match (wants_override, base_member) {
        (true, None) => Err(InheritanceError::OverrideTargetNotFound {
            class_fqn: class_fqn.to_string(),
            member: name,
            span: p.name.span.into(),
        }),
        (true, Some(base)) if !base.modifiers.is_overridable() => {
            Err(InheritanceError::CannotOverrideFinalMember {
                base_fqn: base_fqn.to_string(),
                member: name,
                span: p.name.span.into(),
                base_span: base.span.into(),
            })
        }
        (false, Some(base)) => Err(InheritanceError::MissingOverride {
            class_fqn: class_fqn.to_string(),
            member: name,
            span: p.name.span.into(),
            base_span: base.span.into(),
        }),
        _ => Ok(()),
    }
}

fn check_fun_override(
    ctx: &mut InheritanceCtx<'_, '_>,
    class_fqn: &str,
    base: SuperclassTarget<'_>,
    f: &ast::FunDecl,
) -> Result<(), InheritanceError> {
    let name = ctx.source.slice(f.name.span).to_string();
    let wants_override = f.modifiers.contains(&ast::Modifier::Override);
    let base_member_fqn = format!("{}.{name}", base.fqn);

    let base_overloads = ctx
        .index
        .by_fqn
        .get(&base_member_fqn)
        .map(|syms| syms.fun.as_slice())
        .unwrap_or(&[]);

    let base_owner = OwnerInstantiation {
        fqn: base.fqn,
        nominal: base.nominal,
    };
    let mut matching = Vec::new();
    for overload in base_overloads {
        if fun_decl_matches_overload_signature(
            ctx.source, f, overload, base_owner, ctx.lower, ctx.env,
        )? {
            matching.push(overload);
        }
    }

    match (wants_override, matching.as_slice()) {
        (true, []) => Err(InheritanceError::OverrideTargetNotFound {
            class_fqn: class_fqn.to_string(),
            member: name,
            span: f.name.span.into(),
        }),
        (true, matches) => {
            if let Some(first) = matches.first()
                && matches.iter().all(|o| !o.symbol.modifiers.is_overridable())
            {
                return Err(InheritanceError::OverrideNonOpenMethod {
                    base_fqn: base.fqn.to_string(),
                    member: name,
                    span: f.name.span.into(),
                    base_span: first.symbol.span.into(),
                });
            }
            Ok(())
        }
        (false, matches)
            if !matches.is_empty()
                && matches.iter().all(|o| !o.symbol.modifiers.is_overridable()) =>
        {
            let base_span = matches[0].symbol.span;
            Err(InheritanceError::OverrideNonOpenMethod {
                base_fqn: base.fqn.to_string(),
                member: name,
                span: f.name.span.into(),
                base_span: base_span.into(),
            })
        }
        (false, matches) if !matches.is_empty() => {
            let base_span = matches[0].symbol.span;
            Err(InheritanceError::MissingOverride {
                class_fqn: class_fqn.to_string(),
                member: name,
                span: f.name.span.into(),
                base_span: base_span.into(),
            })
        }
        _ => Ok(()),
    }
}

fn check_virtual_generic_methods(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    type_fqn: &str,
) -> Result<(), InheritanceError> {
    let Some(body) = &decl.body else {
        return Ok(());
    };

    for member in &body.members {
        let ast::TypeMember::Fun(fun) = member else {
            continue;
        };
        if fun.type_params.is_empty() {
            continue;
        }
        let is_virtual = matches!(decl.kind, ast::TypeKind::Interface)
            || fun.modifiers.contains(&ast::Modifier::Open)
            || fun.modifiers.contains(&ast::Modifier::Abstract)
            || fun.modifiers.contains(&ast::Modifier::Override);
        if !is_virtual {
            continue;
        }
        let member = source.slice(fun.name.span).to_string();
        let span = fun
            .type_params
            .first()
            .map(|param| param.name.span)
            .unwrap_or(fun.name.span);
        return Err(InheritanceError::VirtualMethodCannotBeGeneric {
            type_fqn: type_fqn.to_string(),
            member,
            span: span.into(),
        });
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

fn direct_interface_property_override_target_exists(
    source: &SourceFile,
    file: &ast::File,
    decl: &ast::TypeDecl,
    index: &Index,
    property: &ast::PropertyDecl,
) -> bool {
    let name = source.slice(property.name.span).to_string();
    decl.supertypes
        .iter()
        .filter(|st| st.ctor_args_span.is_none())
        .filter_map(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty))
        .any(|interface_fqn| {
            index
                .by_fqn
                .get(&format!("{interface_fqn}.{name}"))
                .and_then(|syms| syms.value.as_ref())
                .is_some()
        })
}

fn direct_interface_fun_override_target_exists(
    ctx: &mut InheritanceCtx<'_, '_>,
    decl: &ast::TypeDecl,
    fun: &ast::FunDecl,
) -> Result<bool, InheritanceError> {
    let name = ctx.source.slice(fun.name.span).to_string();

    for st in decl
        .supertypes
        .iter()
        .filter(|st| st.ctor_args_span.is_none())
    {
        let Some(interface_fqn) = ctx
            .index
            .type_ref_to_fqn_in_file(ctx.source, ctx.file, &st.ty)
        else {
            continue;
        };
        let interface_ty = ctx.lower.lower_type_ref(&st.ty)?;
        let Some(interface_nominal) = nominal_from_type_id(interface_ty, ctx.lower) else {
            continue;
        };
        let Some(syms) = ctx.index.by_fqn.get(&format!("{interface_fqn}.{name}")) else {
            continue;
        };
        let owner = OwnerInstantiation {
            fqn: &interface_fqn,
            nominal: &interface_nominal,
        };
        for overload in &syms.fun {
            if fun_decl_matches_overload_signature(
                ctx.source, fun, overload, owner, ctx.lower, ctx.env,
            )? {
                return Ok(true);
            }
        }
    }

    Ok(false)
}
