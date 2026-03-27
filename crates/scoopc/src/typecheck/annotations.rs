//! 注解系统的最小语义检查（T1002）。
//!
//! 当前阶段目标：
//! - 识别 `annotation class X(...)` 并对其施加最小形态约束（data-only）；
//! - 在 `@Name(...)` 使用处验证 `Name` 必须引用一个注解类（spec §15.2~§15.3）。
//!
//! 非目标（留给后续任务）：
//! - 完整的 target/retention/meta-annotation 规则（T1016）；
//! - 注解参数类型白名单与默认值/必填规则；
//! - 注解在表达式位置的语义（如 `@Suppress(...) expr`）；
//! - `@Extern/@Intrinsic/@NoGC/@Unsafe` 等内建注解的特殊行为（T1003 已覆盖最小门禁；更完整规则见 TODO）。

use std::collections::HashMap;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::Index;
use crate::resolve::ImportTable;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::builtin_annotations::{BuiltinAnnotationFlags, BuiltinAnnotationKind, builtin_annotation_kind};
use super::assignable::is_type_assignable;
use super::lower::TypeLowering;
use super::{AnnotationRetentionPolicy, AnnotationTargetKind, TypeEnv};

#[derive(Debug, Clone, Copy)]
struct AnnotationSite {
    /// 该语法位置的“默认目标”（未写 use-site target 时的含义）。
    primary_target: AnnotationTargetKind,
    /// 该语法位置是否为 `annotation class` 声明（用于限制 `@Target/@Retention` 的合法位置）。
    is_annotation_class_decl: bool,
}

impl AnnotationSite {
    fn new(primary_target: AnnotationTargetKind) -> Self {
        Self {
            primary_target,
            is_annotation_class_decl: false,
        }
    }

    fn annotation_class_decl() -> Self {
        Self {
            primary_target: AnnotationTargetKind::Type,
            is_annotation_class_decl: true,
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
pub enum AnnotationError {
    #[error("注解类必须是 `class`：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_must_be_class))]
    AnnotationClassMustBeClass {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类暂不支持继承/实现接口：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_supertypes_not_supported))]
    AnnotationClassSupertypesNotSupported {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类暂不支持类型体（data-only 仅允许主构造参数）：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_body_not_supported))]
    AnnotationClassBodyNotSupported {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类参数必须是 `val`：{type_fqn}.{param}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_param_must_be_val))]
    AnnotationClassParamMustBeVal {
        type_fqn: String,
        param: String,
        #[label("这里需要写 `val`")]
        span: miette::SourceSpan,
    },

    #[error("未解析的注解类型：{name}")]
    #[diagnostic(code(scoop::typecheck::unresolved_annotation_type))]
    UnresolvedAnnotationType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{name}` 不是注解类（需要 `annotation class`）")]
    #[diagnostic(code(scoop::typecheck::annotation_type_is_not_annotation_class))]
    AnnotationTypeIsNotAnnotationClass {
        name: String,
        #[label("注解使用在这里")]
        use_span: miette::SourceSpan,
        #[label("该类型声明在这里")]
        decl_span: miette::SourceSpan,
    },

    #[error("内建注解 `{annotation}` 只能用于 {allowed}，但这里出现在 {found} 上")]
    #[diagnostic(code(scoop::typecheck::builtin_annotation_invalid_target))]
    BuiltinAnnotationInvalidTarget {
        annotation: String,
        allowed: &'static str,
        found: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("内建注解 `{annotation}` 暂不支持参数")]
    #[diagnostic(code(scoop::typecheck::builtin_annotation_args_not_supported))]
    BuiltinAnnotationArgsNotSupported {
        annotation: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 暂仅支持 0 或 1 个字符串字面量参数")]
    #[diagnostic(code(scoop::typecheck::extern_annotation_args_invalid))]
    ExternAnnotationArgsInvalid {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 函数必须省略函数体（外部实现）：{fun_name}")]
    #[diagnostic(code(scoop::typecheck::extern_fun_must_have_no_body))]
    ExternFunMustHaveNoBody {
        fun_name: String,
        #[label("这里不应有函数体")]
        span: miette::SourceSpan,
    },

    #[error("`@Intrinsic` 函数必须省略函数体（实现由编译器/运行时提供）：{fun_name}")]
    #[diagnostic(code(scoop::typecheck::intrinsic_fun_must_have_no_body))]
    IntrinsicFunMustHaveNoBody {
        fun_name: String,
        #[label("这里不应有函数体")]
        span: miette::SourceSpan,
    },

    #[error("`@Intrinsic` 类型中的成员函数必须省略函数体：{type_fqn}.{fun_name}")]
    #[diagnostic(code(scoop::typecheck::intrinsic_member_fun_must_have_no_body))]
    IntrinsicMemberFunMustHaveNoBody {
        type_fqn: String,
        fun_name: String,
        #[label("这里不应有函数体")]
        span: miette::SourceSpan,
    },

    #[error("非法的 AnnotationTarget：{name}")]
    #[diagnostic(code(scoop::typecheck::invalid_annotation_target_name))]
    InvalidAnnotationTargetName {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{annotation}` 只能用于 {allowed}，但这里出现在 {found} 上")]
    #[diagnostic(code(scoop::typecheck::annotation_invalid_target))]
    AnnotationInvalidTarget {
        annotation: String,
        allowed: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("meta-annotation `{annotation}` 只能用于 `annotation class`，但这里出现在 {found} 上")]
    #[diagnostic(code(scoop::typecheck::meta_annotation_invalid_target))]
    MetaAnnotationInvalidTarget {
        annotation: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("非法的 Retention policy：{policy}（仅支持 \"comptime\" / \"cone\"）")]
    #[diagnostic(code(scoop::typecheck::invalid_annotation_retention_policy))]
    InvalidAnnotationRetentionPolicy {
        policy: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{annotation}` 参数过多（最多 {max} 个）")]
    #[diagnostic(code(scoop::typecheck::annotation_args_too_many))]
    AnnotationArgsTooMany {
        annotation: String,
        max: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{annotation}` 没有名为 `{name}` 的参数")]
    #[diagnostic(code(scoop::typecheck::unknown_annotation_param))]
    UnknownAnnotationParam {
        annotation: String,
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{annotation}` 的参数 `{param}` 被重复赋值")]
    #[diagnostic(code(scoop::typecheck::annotation_arg_duplicate))]
    AnnotationArgDuplicate {
        annotation: String,
        param: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{annotation}` 缺少必填参数 `{param}`")]
    #[diagnostic(code(scoop::typecheck::annotation_arg_missing_required))]
    AnnotationArgMissingRequired {
        annotation: String,
        param: String,
        #[label("注解使用在这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{annotation}` 命名参数之后不能再使用位置参数")]
    #[diagnostic(code(scoop::typecheck::annotation_arg_positional_after_named))]
    AnnotationArgPositionalAfterNamed {
        annotation: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{annotation}` 的参数 `{param}` 必须是编译期常量表达式")]
    #[diagnostic(code(scoop::typecheck::annotation_arg_not_const))]
    AnnotationArgNotConst {
        annotation: String,
        param: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{annotation}` 的参数 `{param}` 类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::annotation_arg_type_mismatch))]
    AnnotationArgTypeMismatch {
        annotation: String,
        param: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

/// 检查一个文件内的注解相关最小规则。
///
/// 说明：
/// - 该检查需要 `Index`/`TypeEnv` 来做“注解名 → 类型符号”的解析与分类；
/// - 它应当在 resolver bodies 之后、其它 typecheck pass 之前执行（尽早报错）。
pub fn check_file_annotations(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), AnnotationError> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    // 文件级注解：`@file:...`
    check_annotation_uses(
        source,
        file,
        index,
        env,
        &mut lower,
        builtins,
        &file.file_annotations,
        AnnotationSite::new(AnnotationTargetKind::Module),
    )?;
    reject_builtin_annotations_on_target(source, &file.file_annotations, "file")?;

    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    &mut lower,
                    builtins,
                    &ta.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Type),
                )?;
                reject_builtin_annotations_on_target(source, &ta.annotations, "typealias")?;
            }
            ast::Item::Fun(fun) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    &mut lower,
                    builtins,
                    &fun.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Function),
                )?;
                check_builtin_annotations_on_fun_decl(source, fun)?;
                check_param_list_annotations(source, file, index, env, &mut lower, builtins, &fun.params)?;
            }
            ast::Item::ExtensionProperty(p) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    &mut lower,
                    builtins,
                    &p.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Property),
                )?;
                reject_builtin_annotations_on_target(source, &p.annotations, "extension property")?;
            }
            ast::Item::Val(v) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    &mut lower,
                    builtins,
                    &v.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Property),
                )?;
                reject_builtin_annotations_on_target(source, &v.annotations, "val/var")?;
            }
            ast::Item::Type(ty) => {
                check_type_decl_annotations(source, file, index, env, &mut lower, builtins, ty, &pkg_prefix)?;
            }
            ast::Item::Object(obj) => {
                check_object_decl_annotations(source, file, index, env, &mut lower, builtins, obj, &pkg_prefix)?;
            }
        }
    }

    Ok(())
}

/// 检查类型声明上的注解，并递归检查其类型体成员（含 nested type/object）。
fn check_type_decl_annotations(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    decl: &ast::TypeDecl,
    prefix: &str,
) -> Result<(), AnnotationError> {
    let local = source.slice(decl.name.span);
    let type_fqn = join_prefix(prefix, local);

    // 1) 注解使用：`@Foo` / `@Foo(...)`。
    let site = if decl.kind == ast::TypeKind::Class && decl.modifiers.contains(&ast::Modifier::Annotation)
    {
        AnnotationSite::annotation_class_decl()
    } else {
        AnnotationSite::new(AnnotationTargetKind::Type)
    };
    check_annotation_uses(source, file, index, env, lower, builtins, &decl.annotations, site)?;
    check_builtin_annotations_on_type_decl(source, decl, &type_fqn)?;

    // 2) 注解类自身的最小形态约束（data-only）。
    if decl.modifiers.contains(&ast::Modifier::Annotation) {
        check_annotation_class_decl_rules(source, decl, &type_fqn)?;
    }

    // 2.5) 主构造参数上的注解（包含 `@param:` / `@property:` / `@field:` 等 use-site target）。
    if let Some(primary_ctor) = &decl.primary_ctor {
        check_param_list_annotations(source, file, index, env, lower, builtins, &primary_ctor.params)?;
    }

    // 3) 递归检查类型体成员（包含 nested types）。
    let Some(body) = &decl.body else {
        return Ok(());
    };
    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    &v.annotations,
                    AnnotationSite::new(AnnotationTargetKind::EnumVariant),
                )?;
                reject_builtin_annotations_on_target(source, &v.annotations, "enum variant")?;
            }
            ast::TypeMember::Property(p) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    &p.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Property),
                )?;
                reject_builtin_annotations_on_target(source, &p.annotations, "property")?;
            }
            ast::TypeMember::SecondaryCtor(ctor) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    &ctor.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Constructor),
                )?;
                reject_builtin_annotations_on_target(source, &ctor.annotations, "constructor")?;
                check_param_list_annotations(source, file, index, env, lower, builtins, &ctor.params)?;
            }
            ast::TypeMember::Fun(fun) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    &fun.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Function),
                )?;
                check_builtin_annotations_on_fun_decl(source, fun)?;
                check_param_list_annotations(source, file, index, env, lower, builtins, &fun.params)?;
            }
            ast::TypeMember::Type(nested) => {
                check_type_decl_annotations(source, file, index, env, lower, builtins, nested, &type_fqn)?;
            }
            ast::TypeMember::Object(obj) => {
                check_object_decl_annotations(source, file, index, env, lower, builtins, obj, &type_fqn)?;
            }
            ast::TypeMember::InitBlock(_b) => {}
        }
    }

    Ok(())
}

/// 检查一组参数上的注解使用（`@Name(...)`）。
fn check_param_list_annotations(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    params: &[ast::Param],
) -> Result<(), AnnotationError> {
    for p in params {
        let site = if p.kind.is_some() {
            // `val/var` 构造参数的主目标是 property；`@param:` 可覆盖到 Param。
            AnnotationSite::new(AnnotationTargetKind::Property)
        } else {
            AnnotationSite::new(AnnotationTargetKind::Param)
        };
        check_annotation_uses(source, file, index, env, lower, builtins, &p.annotations, site)?;
        reject_builtin_annotations_on_target(source, &p.annotations, "param")?;
    }
    Ok(())
}

/// 检查 object 声明上的注解，并递归检查其类型体成员（含 nested type/object）。
fn check_object_decl_annotations(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    obj: &ast::ObjectDecl,
    prefix: &str,
) -> Result<(), AnnotationError> {
    // object 自身的注解使用。
    check_annotation_uses(
        source,
        file,
        index,
        env,
        lower,
        builtins,
        &obj.annotations,
        AnnotationSite::new(AnnotationTargetKind::Type),
    )?;
    check_builtin_annotations_on_object_decl(source, obj)?;

    let Some(body) = &obj.body else {
        return Ok(());
    };

    // 为递归处理 nested type/object 计算容器前缀（与 TypeEnv 的收集规则对齐）。
    let local_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => {
                // parser 会拒绝 `object { ... }`；这里作为健壮性兜底不深入。
                return Ok(());
            }
        },
    };
    let obj_fqn = join_prefix(prefix, &local_name);

    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    &v.annotations,
                    AnnotationSite::new(AnnotationTargetKind::EnumVariant),
                )?;
                reject_builtin_annotations_on_target(source, &v.annotations, "enum variant")?;
            }
            ast::TypeMember::Property(p) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    &p.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Property),
                )?;
                reject_builtin_annotations_on_target(source, &p.annotations, "property")?;
            }
            ast::TypeMember::SecondaryCtor(ctor) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    &ctor.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Constructor),
                )?;
                reject_builtin_annotations_on_target(source, &ctor.annotations, "constructor")?;
            }
            ast::TypeMember::Fun(fun) => {
                check_annotation_uses(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    &fun.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Function),
                )?;
                check_builtin_annotations_on_fun_decl(source, fun)?;
            }
            ast::TypeMember::Type(nested) => {
                check_type_decl_annotations(source, file, index, env, lower, builtins, nested, &obj_fqn)?;
            }
            ast::TypeMember::Object(nested) => {
                check_object_decl_annotations(source, file, index, env, lower, builtins, nested, &obj_fqn)?;
            }
            ast::TypeMember::InitBlock(_b) => {}
        }
    }

    Ok(())
}

/// 注解类（`annotation class`）的最小形态约束（data-only）。
fn check_annotation_class_decl_rules(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    type_fqn: &str,
) -> Result<(), AnnotationError> {
    // spec §15.2：语法为 `annotation class ...`，当前阶段只支持 class。
    if decl.kind != ast::TypeKind::Class {
        return Err(AnnotationError::AnnotationClassMustBeClass {
            type_fqn: type_fqn.to_string(),
            span: decl.name.span.into(),
        });
    }

    // 当前阶段（T1002）：annotation class 作为 data-only 容器，不允许 implements/extends。
    if let Some(st) = decl.supertypes.first() {
        return Err(AnnotationError::AnnotationClassSupertypesNotSupported {
            type_fqn: type_fqn.to_string(),
            span: st.span.into(),
        });
    }

    // 当前阶段（T1002）：不解析/不支持注解类的类型体成员（方法/属性等）。
    if let Some(body) = &decl.body {
        return Err(AnnotationError::AnnotationClassBodyNotSupported {
            type_fqn: type_fqn.to_string(),
            span: body.span.into(),
        });
    }

    // spec §15.2：所有参数必须是 `val`（immutable）。
    if let Some(primary_ctor) = &decl.primary_ctor {
        for p in &primary_ctor.params {
            if p.kind != Some(ast::ValKind::Val) {
                let param = source.slice(p.name.span).to_string();
                return Err(AnnotationError::AnnotationClassParamMustBeVal {
                    type_fqn: type_fqn.to_string(),
                    param,
                    span: p.name.span.into(),
                });
            }
        }
    }

    Ok(())
}

/// 批量检查一组注解使用（`@Name(...)`）。
fn check_annotation_uses(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    annotations: &[ast::AnnotationUse],
    site: AnnotationSite,
) -> Result<(), AnnotationError> {
    for a in annotations {
        check_one_annotation_use(source, file, index, env, lower, builtins, a, site)?;
    }
    Ok(())
}

/// 检查单个注解使用：解析注解名并确认其引用一个注解类。
fn check_one_annotation_use(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    ann: &ast::AnnotationUse,
    site: AnnotationSite,
) -> Result<(), AnnotationError> {
    let (name, name_span) = annotation_name_and_span(source, ann);

    // T1003：内建注解（`@Unsafe/@NoGC/@Extern/@Intrinsic`）由编译器识别，
    // 不要求存在对应的 `annotation class` 声明。
    if builtin_annotation_kind(source, ann).is_some() {
        return Ok(());
    }

    // 复用 Index 的“按 package/import 规则解析类型名”的逻辑来解析注解类型。
    let ty = ast::TypeRef::Path(ast::TypePath {
        span: ann.span,
        segments: ann.path.clone(),
        args: Vec::new(),
    });

    let Some(fqn) = index.type_ref_to_fqn_in_file(source, file, &ty) else {
        return Err(AnnotationError::UnresolvedAnnotationType {
            name,
            span: name_span.into(),
        });
    };

    let Some(sym) = env.type_symbol(&fqn) else {
        return Err(AnnotationError::UnresolvedAnnotationType {
            name,
            span: name_span.into(),
        });
    };

    if !sym.is_annotation_class {
        return Err(AnnotationError::AnnotationTypeIsNotAnnotationClass {
            name: fqn,
            use_span: name_span.into(),
            decl_span: sym.span.into(),
        });
    }

    let effective_target = effective_annotation_target(source, ann, site.primary_target);

    // T1016a：meta-annotations 的合法位置与最小参数检查。
    if fqn == "scoop.core.Target" {
        if !site.is_annotation_class_decl {
            return Err(AnnotationError::MetaAnnotationInvalidTarget {
                annotation: "@Target".to_string(),
                found: effective_target.as_str().to_string(),
                span: name_span.into(),
            });
        }
        check_target_annotation_args(source, ann)?;
        return Ok(());
    }
    if fqn == "scoop.core.Retention" {
        if !site.is_annotation_class_decl {
            return Err(AnnotationError::MetaAnnotationInvalidTarget {
                annotation: "@Retention".to_string(),
                found: effective_target.as_str().to_string(),
                span: name_span.into(),
            });
        }
        check_retention_annotation_args(source, ann)?;
        return Ok(());
    }

    // T1016a：若注解类声明了 `@Target(...)`，则在使用点强制执行目标限制。
    if let Some(allowed) = &sym.annotation_targets {
        if !allowed.contains(&effective_target) {
            return Err(AnnotationError::AnnotationInvalidTarget {
                annotation: fqn,
                allowed: join_target_list(allowed),
                found: effective_target.as_str().to_string(),
                span: name_span.into(),
            });
        }
    }

    // T1019：注解参数的“类型匹配 + 编译期常量”检查。
    check_annotation_args(source, file, index, env, lower, builtins, &fqn, sym, ann)?;

    Ok(())
}

fn check_target_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    for arg in &ann.args {
        let Some((variant_name, variant_span)) = extract_annotation_target_variant(source, &arg.value)
        else {
            continue;
        };

        if !is_valid_annotation_target_variant(&variant_name) {
            return Err(AnnotationError::InvalidAnnotationTargetName {
                name: variant_name,
                span: variant_span.into(),
            });
        }
    }
    Ok(())
}

fn check_retention_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    let Some(arg) = ann.args.first() else {
        // 早期阶段：不做“必填参数/默认值”强制，只在存在参数时做合法性检查。
        return Ok(());
    };
    let Some(policy_text) = extract_string_literal_text(source, &arg.value) else {
        return Ok(());
    };
    if AnnotationRetentionPolicy::parse(policy_text.as_str()).is_none() {
        return Err(AnnotationError::InvalidAnnotationRetentionPolicy {
            policy: policy_text,
            span: arg.value.span.into(),
        });
    }
    Ok(())
}

fn check_annotation_args(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    annotation_fqn: &str,
    sym: &super::TypeSymbol,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    let params = sym.annotation_params.as_slice();
    if params.is_empty() {
        if ann.args.is_empty() {
            return Ok(());
        }
        return Err(AnnotationError::AnnotationArgsTooMany {
            annotation: annotation_fqn.to_string(),
            max: 0,
            span: ann.span.into(),
        });
    }

    let mut idx_of: HashMap<String, usize> = HashMap::new();
    for (idx, p) in params.iter().enumerate() {
        idx_of.insert(p.name.clone(), idx);
    }

    let mut assigned: Vec<Option<&ast::AnnotationArg>> = vec![None; params.len()];
    let mut next_positional = 0usize;
    let mut seen_named = false;

    for arg in &ann.args {
        match &arg.name {
            Some(name_id) => {
                seen_named = true;
                let name = name_id.text(source).to_string();
                let Some(&param_idx) = idx_of.get(&name) else {
                    return Err(AnnotationError::UnknownAnnotationParam {
                        annotation: annotation_fqn.to_string(),
                        name,
                        span: name_id.span.into(),
                    });
                };
                if assigned[param_idx].is_some() {
                    return Err(AnnotationError::AnnotationArgDuplicate {
                        annotation: annotation_fqn.to_string(),
                        param: params[param_idx].name.clone(),
                        span: name_id.span.into(),
                    });
                }
                assigned[param_idx] = Some(arg);
            }
            None => {
                if seen_named {
                    return Err(AnnotationError::AnnotationArgPositionalAfterNamed {
                        annotation: annotation_fqn.to_string(),
                        span: arg.span.into(),
                    });
                }
                if next_positional >= params.len() {
                    return Err(AnnotationError::AnnotationArgsTooMany {
                        annotation: annotation_fqn.to_string(),
                        max: params.len(),
                        span: arg.span.into(),
                    });
                }
                assigned[next_positional] = Some(arg);
                next_positional += 1;
            }
        }
    }

    for (idx, p) in params.iter().enumerate() {
        if assigned[idx].is_none() && !p.has_default {
            return Err(AnnotationError::AnnotationArgMissingRequired {
                annotation: annotation_fqn.to_string(),
                param: p.name.clone(),
                span: ann.span.into(),
            });
        }
    }

    for (idx, maybe_arg) in assigned.iter().enumerate() {
        let Some(arg) = maybe_arg else {
            continue;
        };

        let expected_ty = match lower.lower_type_ref_in_decl_file(&sym.decl_file, &params[idx].ty) {
            Ok(ty) => ty,
            Err(_e) => {
                // 更精确的类型诊断由 `check_file_type_refs`/`TypeLowerError` 给出；
                // 这里保持注解检查阶段的健壮性，不在此重复报错。
                continue;
            }
        };

        let found_ty = infer_annotation_const_expr_type(
            source,
            file,
            index,
            env,
            lower,
            builtins,
            &arg.value,
            annotation_fqn,
            params[idx].name.as_str(),
        )?;

        if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
            return Err(AnnotationError::AnnotationArgTypeMismatch {
                annotation: annotation_fqn.to_string(),
                param: params[idx].name.clone(),
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: arg.value.span.into(),
            });
        }
    }

    Ok(())
}

fn infer_annotation_const_expr_type(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    expr: &ast::Expr,
    annotation_fqn: &str,
    param_name: &str,
) -> Result<TypeId, AnnotationError> {
    let not_const = || AnnotationError::AnnotationArgNotConst {
        annotation: annotation_fqn.to_string(),
        param: param_name.to_string(),
        span: expr.span.into(),
    };

    match &expr.kind {
        ast::ExprKind::IntLit => Ok(builtins.int),
        ast::ExprKind::StringLit => Ok(builtins.string),
        ast::ExprKind::UnitLit => Ok(builtins.unit),
        ast::ExprKind::InterpolatedString { .. } => Err(not_const()),
        ast::ExprKind::Ident(id) => match source.slice(id.span) {
            "true" | "false" => Ok(builtins.bool_),
            _ => Err(not_const()),
        },
        ast::ExprKind::Unary { op, expr: inner, .. } => {
            let operand_ty = infer_annotation_const_expr_type(
                source,
                file,
                index,
                env,
                lower,
                builtins,
                inner.as_ref(),
                annotation_fqn,
                param_name,
            )?;
            match op {
                ast::UnaryOp::Not => {
                    if operand_ty == builtins.bool_ {
                        Ok(builtins.bool_)
                    } else {
                        Err(not_const())
                    }
                }
                ast::UnaryOp::Neg | ast::UnaryOp::BitNot => {
                    if is_integer_type_for_const_expr(operand_ty, lower, builtins) {
                        Ok(operand_ty)
                    } else {
                        Err(not_const())
                    }
                }
            }
        }
        ast::ExprKind::Binary { lhs, op, rhs, .. } => {
            let lhs_ty = infer_annotation_const_expr_type(
                source,
                file,
                index,
                env,
                lower,
                builtins,
                lhs.as_ref(),
                annotation_fqn,
                param_name,
            )?;
            let rhs_ty = infer_annotation_const_expr_type(
                source,
                file,
                index,
                env,
                lower,
                builtins,
                rhs.as_ref(),
                annotation_fqn,
                param_name,
            )?;

            match op {
                ast::BinaryOp::Add
                | ast::BinaryOp::Sub
                | ast::BinaryOp::Mul
                | ast::BinaryOp::Div
                | ast::BinaryOp::Rem
                | ast::BinaryOp::BitAnd
                | ast::BinaryOp::BitXor
                | ast::BinaryOp::BitOr => unify_integer_operands_for_const_expr(lhs, lhs_ty, rhs, rhs_ty, lower, builtins)
                    .ok_or_else(not_const),

                ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                    if is_integer_type_for_const_expr(lhs_ty, lower, builtins) && rhs_ty == builtins.int {
                        Ok(lhs_ty)
                    } else {
                        Err(not_const())
                    }
                }

                ast::BinaryOp::Lt
                | ast::BinaryOp::Le
                | ast::BinaryOp::Gt
                | ast::BinaryOp::Ge => {
                    if unify_integer_operands_for_const_expr(lhs, lhs_ty, rhs, rhs_ty, lower, builtins).is_some() {
                        Ok(builtins.bool_)
                    } else {
                        Err(not_const())
                    }
                }

                ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
                    if lhs_ty == builtins.bool_ && rhs_ty == builtins.bool_ {
                        return Ok(builtins.bool_);
                    }
                    if unify_integer_operands_for_const_expr(lhs, lhs_ty, rhs, rhs_ty, lower, builtins).is_some() {
                        return Ok(builtins.bool_);
                    }
                    Err(not_const())
                }

                ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                    if lhs_ty == builtins.bool_ && rhs_ty == builtins.bool_ {
                        Ok(builtins.bool_)
                    } else {
                        Err(not_const())
                    }
                }

                ast::BinaryOp::Elvis => Err(not_const()),
            }
        }
        ast::ExprKind::MemberAccess { .. } => infer_enum_unit_variant_const_type(source, file, index, env, lower, expr)
            .ok_or_else(not_const),
        ast::ExprKind::ArrayLit { elements } => {
            let first = elements.first().ok_or_else(not_const)?;
            let mut elem_ty = infer_annotation_const_expr_type(
                source,
                file,
                index,
                env,
                lower,
                builtins,
                first,
                annotation_fqn,
                param_name,
            )?;

            for e in elements.iter().skip(1) {
                let ty = infer_annotation_const_expr_type(
                    source,
                    file,
                    index,
                    env,
                    lower,
                    builtins,
                    e,
                    annotation_fqn,
                    param_name,
                )?;

                if ty == elem_ty {
                    continue;
                }

                // 允许 `Int` 字面量元素“适配”到其它整数类型（与表达式 typecheck 的最小规则对齐）。
                if matches!(&e.kind, ast::ExprKind::IntLit)
                    && is_integer_type_for_const_expr(elem_ty, lower, builtins)
                    && is_integer_type_for_const_expr(ty, lower, builtins)
                {
                    continue;
                }

                if matches!(&first.kind, ast::ExprKind::IntLit)
                    && is_integer_type_for_const_expr(elem_ty, lower, builtins)
                    && is_integer_type_for_const_expr(ty, lower, builtins)
                {
                    elem_ty = ty;
                    continue;
                }

                return Err(not_const());
            }

            lower
                .lower_type_fqn_with_args("scoop.core.Array".to_string(), vec![elem_ty], expr.span)
                .map_err(|_e| not_const())
        }
        ast::ExprKind::ClassLit { ty } => {
            // 当前阶段：class literal 仅作为“编译期可用的类型名常量”存在。
            // 这里做最小保证：类型引用必须可解析（存在性由 Index/TypeEnv 决定）。
            if index.type_ref_to_fqn_in_file(source, file, ty).is_none() {
                return Err(not_const());
            }
            Ok(builtins.string)
        }
        _ => Err(not_const()),
    }
}

fn infer_enum_unit_variant_const_type(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    lower: &mut TypeLowering<'_>,
    expr: &ast::Expr,
) -> Option<TypeId> {
    let mut segs: Vec<(String, Span)> = Vec::new();
    if !collect_member_access_path(source, expr, &mut segs) {
        return None;
    }
    if segs.len() < 2 {
        return None;
    }

    let (variant_name, _variant_span) = segs.last().cloned()?;
    let type_segs = &segs[..segs.len() - 1];
    let first = type_segs.first()?;
    let last = type_segs.last()?;

    let path = ast::TypePath {
        span: Span::new(first.1.start, last.1.end),
        segments: type_segs
            .iter()
            .map(|(_text, span)| ast::Ident::new(*span))
            .collect(),
        args: Vec::new(),
    };
    let ty_ref = ast::TypeRef::Path(path);
    let enum_fqn = index.type_ref_to_fqn_in_file(source, file, &ty_ref)?;

    let decl = env.enum_decl(&enum_fqn)?;
    let ok = decl
        .variants
        .iter()
        .any(|v| v.name == variant_name && v.fields.is_empty());
    if !ok {
        return None;
    }

    lower
        .lower_type_fqn_with_args(enum_fqn, Vec::new(), expr.span)
        .ok()
}

fn is_integer_type_for_const_expr(ty: TypeId, lower: &TypeLowering<'_>, builtins: BuiltinTypes) -> bool {
    if ty == builtins.int || ty == builtins.uint {
        return true;
    }

    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::IntN(_) | ValueTypeKind::UIntN(_)) => true,
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => matches!(
            nominal.fqn.as_str(),
            "scoop.core.Int8"
                | "scoop.core.Int16"
                | "scoop.core.Int32"
                | "scoop.core.Int64"
                | "scoop.core.UInt8"
                | "scoop.core.UInt16"
                | "scoop.core.UInt32"
                | "scoop.core.UInt64"
        ),
        _ => false,
    }
}

fn unify_integer_operands_for_const_expr(
    lhs: &ast::Expr,
    lhs_ty: TypeId,
    rhs: &ast::Expr,
    rhs_ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<TypeId> {
    if lhs_ty == rhs_ty && is_integer_type_for_const_expr(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    if matches!(&lhs.kind, ast::ExprKind::IntLit) && is_integer_type_for_const_expr(rhs_ty, lower, builtins) {
        return Some(rhs_ty);
    }
    if matches!(&rhs.kind, ast::ExprKind::IntLit) && is_integer_type_for_const_expr(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    None
}

fn effective_annotation_target(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
    primary: AnnotationTargetKind,
) -> AnnotationTargetKind {
    let Some(t) = ann.use_site_target.as_ref() else {
        return primary;
    };
    match t.text(source) {
        "file" => AnnotationTargetKind::Module,
        "property" => AnnotationTargetKind::Property,
        "field" => AnnotationTargetKind::Field,
        "param" => AnnotationTargetKind::Param,
        // 当前阶段（T1014）：`get:`/`set:` 仅做语法存储；这里按 property 处理以避免过早引入 accessor 目标。
        "get" | "set" => AnnotationTargetKind::Property,
        _ => primary,
    }
}

fn extract_annotation_target_variant(source: &SourceFile, expr: &ast::Expr) -> Option<(String, Span)> {
    let mut segs: Vec<(String, Span)> = Vec::new();
    if !collect_member_access_path(source, expr, &mut segs) {
        return None;
    }
    if segs.len() < 2 {
        return None;
    }

    // 允许：`AnnotationTarget.Field` / `scoop.core.AnnotationTarget.Field`
    let penultimate = segs.get(segs.len().saturating_sub(2))?.0.as_str();
    if penultimate != "AnnotationTarget" {
        return None;
    }
    segs.last().cloned()
}

fn collect_member_access_path(
    source: &SourceFile,
    expr: &ast::Expr,
    out: &mut Vec<(String, Span)>,
) -> bool {
    match &expr.kind {
        ast::ExprKind::Ident(id) => {
            out.push((source.slice(id.span).to_string(), id.span));
            true
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            if !collect_member_access_path(source, receiver, out) {
                return false;
            }
            out.push((source.slice(member.span).to_string(), member.span));
            true
        }
        _ => false,
    }
}

fn is_valid_annotation_target_variant(name: &str) -> bool {
    matches!(
        name,
        "Function"
            | "Property"
            | "Field"
            | "Param"
            | "Type"
            | "Constructor"
            | "LocalVariable"
            | "Expression"
            | "Module"
            | "TypeParam"
            | "EnumVariant"
    )
}

fn join_target_list(targets: &[AnnotationTargetKind]) -> String {
    targets
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn extract_string_literal_text<'a>(source: &'a SourceFile, expr: &ast::Expr) -> Option<String> {
    if !matches!(expr.kind, ast::ExprKind::StringLit) {
        return None;
    }
    let raw = source.slice(expr.span);
    let s = raw
        .strip_prefix("\"\"\"")
        .and_then(|t| t.strip_suffix("\"\"\""))
        .or_else(|| raw.strip_prefix('\"').and_then(|t| t.strip_suffix('\"')))
        .unwrap_or(raw);
    Some(s.to_string())
}

fn reject_builtin_annotations_on_target(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
    found: &'static str,
) -> Result<(), AnnotationError> {
    for ann in annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        let (_, name_span) = annotation_name_and_span(source, ann);
        return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
            annotation: format!("@{}", kind.name()),
            allowed: kind.allowed_targets_hint(),
            found,
            span: name_span.into(),
        });
    }
    Ok(())
}

fn check_builtin_annotations_on_fun_decl(
    source: &SourceFile,
    fun: &ast::FunDecl,
) -> Result<(), AnnotationError> {
    let flags = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations);

    // 1) `@Unsafe/@NoGC/@Intrinsic` 当前不支持参数；`@Extern` 允许最多一个字符串字面量参数。
    for ann in &fun.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        match kind {
            BuiltinAnnotationKind::Extern => {
                if ann.args.is_empty() {
                    continue;
                }
                if ann.args.len() != 1 {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: ann.span.into(),
                    });
                }
                let arg = &ann.args[0];
                if arg.name.is_some() {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: arg.span.into(),
                    });
                }
                if !matches!(arg.value.kind, ast::ExprKind::StringLit) {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: arg.span.into(),
                    });
                }
            }
            BuiltinAnnotationKind::Unsafe
            | BuiltinAnnotationKind::NoGC
            | BuiltinAnnotationKind::Intrinsic => {
                if !ann.args.is_empty() {
                    let (_, name_span) = annotation_name_and_span(source, ann);
                    return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
                        annotation: format!("@{}", kind.name()),
                        span: name_span.into(),
                    });
                }
            }
        }
    }

    // 2) `@Extern/@Intrinsic`：声明必须省略函数体（实现由外部/编译器提供）。
    if flags.is_extern {
        if let ast::FunBody::Block(b) = &fun.body {
            let fun_name = source.slice(fun.name.span).to_string();
            return Err(AnnotationError::ExternFunMustHaveNoBody {
                fun_name,
                span: b.span.into(),
            });
        }
    }
    if flags.is_intrinsic {
        if let ast::FunBody::Block(b) = &fun.body {
            let fun_name = source.slice(fun.name.span).to_string();
            return Err(AnnotationError::IntrinsicFunMustHaveNoBody {
                fun_name,
                span: b.span.into(),
            });
        }
    }

    Ok(())
}

fn check_builtin_annotations_on_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    type_fqn: &str,
) -> Result<(), AnnotationError> {
    let flags = BuiltinAnnotationFlags::from_annotations(source, &decl.annotations);

    // type 声明目前只允许 `@Intrinsic`；其它内建注解的 target 语义留到后续任务补齐。
    for ann in &decl.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        if kind != BuiltinAnnotationKind::Intrinsic {
            let (_, name_span) = annotation_name_and_span(source, ann);
            return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                annotation: format!("@{}", kind.name()),
                allowed: kind.allowed_targets_hint(),
                found: "type",
                span: name_span.into(),
            });
        }
        if !ann.args.is_empty() {
            let (_, name_span) = annotation_name_and_span(source, ann);
            return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
                annotation: format!("@{}", kind.name()),
                span: name_span.into(),
            });
        }
    }

    // spec §15.7：intrinsic declarations have signatures but no bodies。
    // 当前阶段只对“成员函数是否带 body”做最小门禁（不涉及 codegen lowering）。
    if flags.is_intrinsic {
        let Some(body) = &decl.body else {
            return Ok(());
        };
        for m in &body.members {
            let ast::TypeMember::Fun(fun) = m else {
                continue;
            };
            if let ast::FunBody::Block(b) = &fun.body {
                let fun_name = source.slice(fun.name.span).to_string();
                return Err(AnnotationError::IntrinsicMemberFunMustHaveNoBody {
                    type_fqn: type_fqn.to_string(),
                    fun_name,
                    span: b.span.into(),
                });
            }
        }
    }

    Ok(())
}

fn check_builtin_annotations_on_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
) -> Result<(), AnnotationError> {
    for ann in &obj.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        if kind != BuiltinAnnotationKind::Extern {
            let (_, name_span) = annotation_name_and_span(source, ann);
            return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                annotation: format!("@{}", kind.name()),
                allowed: kind.allowed_targets_hint(),
                found: "object",
                span: name_span.into(),
            });
        }
        // `@Extern` 目前仅允许：无参或单字符串字面量（保留给后续 FFI/链接语义）。
        if ann.args.is_empty() {
            continue;
        }
        if ann.args.len() != 1 {
            return Err(AnnotationError::ExternAnnotationArgsInvalid {
                span: ann.span.into(),
            });
        }
        let arg = &ann.args[0];
        if arg.name.is_some() || !matches!(arg.value.kind, ast::ExprKind::StringLit) {
            return Err(AnnotationError::ExternAnnotationArgsInvalid {
                span: arg.span.into(),
            });
        }
    }

    Ok(())
}

/// 从 AST 的 `AnnotationUse` 构造用于诊断与解析的“名字 + 标注 span”。
fn annotation_name_and_span(source: &SourceFile, ann: &ast::AnnotationUse) -> (String, Span) {
    let name = ann
        .path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>()
        .join(".");
    let span = ann.path.first().map(|id| id.span).unwrap_or(ann.span);
    (name, span)
}

/// 计算当前文件的 package 前缀（`package a.b.c` → `"a.b.c"`）。
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

/// 拼接 FQN：`prefix` 为空时返回 `name`，否则返回 `"{prefix}.{name}"`。
fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}
