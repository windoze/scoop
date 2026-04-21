//! 注解系统的 compile-time marker 语义检查。
//!
//! 当前阶段目标（T4012a）：
//! - 把 `annotation` 收口为 **compile-time markers only**，而不是一般 nominal type 能力的延伸；
//! - 识别 `annotation class X(...)` 并对其施加 data-only marker 约束；
//! - 在 `@Name(...)` 使用处验证 `Name` 必须引用一个注解类（spec §15.2~§15.3）。
//!
//! 当前合同：
//! - `annotation` 关键字只服务于 `annotation class`；
//! - annotation class 只允许以主构造 `val` 参数承载编译期数据；
//! - annotation class 不引入继承、运行时对象模型或额外控制流语义。
//!
//! 非目标（留给后续任务）：
//! - 完整的 built-in annotation behavior（`@Deprecated/@AllowIntrinsic/@Suppress/...`）；
//! - 注解在表达式位置的语义（如 `@Suppress(...) expr`）；
//! - 更丰富的 metaprogramming / reflection surface。

use std::collections::HashMap;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::ImportTable;
use crate::resolve::Index;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::int_literal::parse_int_literal;
use crate::ty::{BuiltinTypes, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::assignable::is_type_assignable;
use super::builtin_annotations::{
    BuiltinAnnotationFlags, BuiltinAnnotationKind, builtin_annotation_kind,
};
use super::lower::TypeLowering;
use super::{AnnotationRetentionPolicy, AnnotationTargetKind, TypeEnv};

#[derive(Debug, Clone, Copy)]
struct AnnotationSite {
    /// 该语法位置的“默认目标”（未写 use-site target 时的含义）。
    primary_target: AnnotationTargetKind,
    /// 该语法位置是否为 `annotation class` 声明（用于限制 `@Target/@Retention` 的合法位置）。
    is_annotation_class_decl: bool,
}

type ParsedCLayoutArgs = (Option<u64>, Option<Span>, Option<u64>, Option<Span>);

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

#[derive(Clone, Copy)]
struct AnnotationCheckContext<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    env: &'a TypeEnv,
}

#[derive(Debug, Error, Diagnostic)]
pub enum AnnotationError {
    #[error("`annotation` 关键字只能用于 `annotation class` 声明，但这里出现在 {found} 上")]
    #[diagnostic(code(scoop::typecheck::annotation_modifier_invalid_target))]
    AnnotationModifierInvalidTarget {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类必须是 `class`：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_must_be_class))]
    AnnotationClassMustBeClass {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类不支持修饰符 `{modifier}`：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_modifier_not_supported))]
    AnnotationClassModifierNotSupported {
        type_fqn: String,
        modifier: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类暂不支持 effect 参数：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_effect_param_not_supported))]
    AnnotationClassEffectParamNotSupported {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类暂不支持 where 子句：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_where_clause_not_supported))]
    AnnotationClassWhereClauseNotSupported {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类暂不支持类型参数：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_type_params_not_supported))]
    AnnotationClassTypeParamsNotSupported {
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

    #[error("`@Extern` 仅支持：无参 / 单个字符串位置参数 / 命名参数 `name`、`lib`（字符串字面量）")]
    #[diagnostic(code(scoop::typecheck::extern_annotation_args_invalid))]
    ExternAnnotationArgsInvalid {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@CallingConvention` 仅支持：单个字符串位置参数 / 命名参数 `name`（字符串字面量）")]
    #[diagnostic(code(scoop::typecheck::calling_convention_annotation_args_invalid))]
    CallingConventionAnnotationArgsInvalid {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的 calling convention：{name}（当前仅支持默认 C ABI：\"c\"/\"cdecl\"）")]
    #[diagnostic(code(scoop::typecheck::calling_convention_not_supported))]
    CallingConventionNotSupported {
        name: String,
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

    #[error("`@Extern` 顶层变量声明必须省略 initializer：{var_name}")]
    #[diagnostic(code(scoop::typecheck::extern_var_initializer_not_allowed))]
    ExternVarInitializerNotAllowed {
        var_name: String,
        #[label("这里不应有 initializer")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 顶层变量类型必须是 GC-free 值类型（不允许直接/间接包含 GC 引用）：{found}")]
    #[diagnostic(code(scoop::typecheck::extern_var_type_must_be_gc_free))]
    ExternVarTypeMustBeGcFree {
        found: String,
        #[label("这里的类型不是 GC-free 值类型")]
        span: miette::SourceSpan,
    },

    #[error(
        "`@Extern` 函数的 ABI 签名必须是 GC-free 值类型（不允许直接/间接包含 GC 引用）：{found}；请使用 `GC.pin/unpin` + `scoop.unsafe.Ptr<T>`（或 handle）桥接"
    )]
    #[diagnostic(code(scoop::typecheck::extern_fun_signature_must_be_gc_free))]
    ExternFunSignatureMustBeGcFree {
        found: String,
        #[label("这里的类型不是 GC-free 值类型")]
        span: miette::SourceSpan,
    },

    #[error("顶层 `var` 必须显式标注 `@ThreadLocal` 或 `@Global`：{var_name}")]
    #[diagnostic(code(scoop::typecheck::top_level_var_requires_threadlocal_or_global))]
    TopLevelVarRequiresThreadLocalOrGlobal {
        var_name: String,
        #[label("这里的顶层 var 需要标注 `@ThreadLocal` 或 `@Global`")]
        span: miette::SourceSpan,
    },

    #[error("顶层 `var` 类型必须是 GC-free 值类型（不允许直接/间接包含 GC 引用）：{found}")]
    #[diagnostic(code(scoop::typecheck::top_level_var_type_must_be_gc_free))]
    TopLevelVarTypeMustBeGcFree {
        found: String,
        #[label("这里的类型不是 GC-free 值类型")]
        span: miette::SourceSpan,
    },

    #[error("`@CLayout` struct 必须是 GC-free 值类型（不允许直接/间接包含 GC 引用）：{struct_fqn}")]
    #[diagnostic(code(scoop::typecheck::clayout_struct_must_be_gc_free))]
    CLayoutStructMustBeGcFree {
        struct_fqn: String,
        #[label("这里的 struct 不是 GC-free")]
        span: miette::SourceSpan,
    },

    #[error("`@CLayout` 参数 `{param}` 必须是整数字面量（当前阶段仅支持 int literal）")]
    #[diagnostic(code(scoop::typecheck::clayout_param_must_be_int_literal))]
    CLayoutParamMustBeIntLiteral {
        param: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@CLayout(packed = ...)` 必须是正的 2 的幂且 ≤ 16（得到 {value}）")]
    #[diagnostic(code(scoop::typecheck::clayout_packed_value_not_supported))]
    CLayoutPackedValueNotSupported {
        value: u64,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@CLayout(aligned = ...)` 必须是正的 2 的幂（得到 {value}）")]
    #[diagnostic(code(scoop::typecheck::clayout_aligned_value_invalid))]
    CLayoutAlignedValueInvalid {
        value: u64,
        #[label("这里")]
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
    let ctx = AnnotationCheckContext {
        source,
        file,
        index,
        env,
    };
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    // 文件级注解：`@file:...`
    check_annotation_uses(
        ctx,
        &mut lower,
        builtins,
        &file.file_annotations,
        AnnotationSite::new(AnnotationTargetKind::Module),
    )?;
    reject_builtin_annotations_on_target(source, &file.file_annotations, "file")?;

    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                reject_annotation_modifier_on_non_type_target(
                    &ta.modifiers,
                    "typealias",
                    ta.name.span,
                )?;
                check_annotation_uses(
                    ctx,
                    &mut lower,
                    builtins,
                    &ta.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Type),
                )?;
                check_builtin_annotations_on_type_alias_decl(source, ta)?;
            }
            ast::Item::Fun(fun) => {
                reject_annotation_modifier_on_non_type_target(
                    &fun.modifiers,
                    "函数",
                    fun.name.span,
                )?;
                check_annotation_uses(
                    ctx,
                    &mut lower,
                    builtins,
                    &fun.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Function),
                )?;
                check_builtin_annotations_on_fun_decl(source, fun, &mut lower)?;
                check_param_list_annotations(ctx, &mut lower, builtins, &fun.params)?;
            }
            ast::Item::ExtensionProperty(p) => {
                reject_annotation_modifier_on_non_type_target(
                    &p.modifiers,
                    "扩展属性",
                    p.name.span,
                )?;
                check_annotation_uses(
                    ctx,
                    &mut lower,
                    builtins,
                    &p.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Property),
                )?;
                reject_builtin_annotations_on_target(source, &p.annotations, "extension property")?;
            }
            ast::Item::Val(v) => {
                reject_annotation_modifier_on_non_type_target(
                    &v.modifiers,
                    "顶层属性",
                    v.name().map(|name| name.span).unwrap_or(v.span),
                )?;
                check_annotation_uses(
                    ctx,
                    &mut lower,
                    builtins,
                    &v.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Property),
                )?;
                check_builtin_annotations_on_top_level_val_decl(source, v, &mut lower)?;
                check_top_level_var_storage_and_gc_free(source, file, index, v, &mut lower)?;
            }
            ast::Item::Type(ty) => {
                check_type_decl_annotations(ctx, &mut lower, builtins, ty, &pkg_prefix)?;
            }
            ast::Item::Object(obj) => {
                check_object_decl_annotations(ctx, &mut lower, builtins, obj, &pkg_prefix)?;
            }
            // T1220a：package-level comptime if 在进入 typecheck 之前应被裁剪（TODO T1220b）。
            ast::Item::ComptimeIf(_ci) => {}
        }
    }

    Ok(())
}

/// 检查类型声明上的注解，并递归检查其类型体成员（含 nested type/object）。
fn check_type_decl_annotations(
    ctx: AnnotationCheckContext<'_>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    decl: &ast::TypeDecl,
    prefix: &str,
) -> Result<(), AnnotationError> {
    let local = ctx.source.slice(decl.name.span);
    let type_fqn = join_prefix(prefix, local);

    // 0) `annotation class` 的 declaration shape 先于注解 use-site 检查：
    //    若声明头本身非法，应优先给出“annotation 不是一般 nominal type”的诊断，
    //    避免 `@Target/@Retention` 等 meta-annotation 先报次级错误。
    if decl.modifiers.contains(&ast::Modifier::Annotation) {
        check_annotation_class_decl_rules(ctx.source, decl, &type_fqn)?;
    }

    // 1) 注解使用：`@Foo` / `@Foo(...)`。
    let site = if decl.modifiers.contains(&ast::Modifier::Annotation) {
        AnnotationSite::annotation_class_decl()
    } else {
        AnnotationSite::new(AnnotationTargetKind::Type)
    };
    check_annotation_uses(ctx, lower, builtins, &decl.annotations, site)?;
    check_builtin_annotations_on_type_decl(ctx.source, decl, &type_fqn)?;

    // 1.5) `@CLayout(aligned, packed)`：GC-free struct 的 ABI 布局控制（spec §15.5.2）。
    //
    // 说明：
    // - `@CLayout` 是 sysroot 声明的内建注解类（`scoop.core.CLayout`），但其约束是“结构体布局语义”，
    //   因此不放进 `BuiltinAnnotationKind`（它们主要是执行模型/门禁类注解）。
    // - 这里在 typecheck 阶段做最小门禁与参数合法性检查；后端（LLVM）会消费这些参数生成 packed/aligned layout。
    check_clayout_struct_decl(ctx.source, ctx.file, ctx.index, decl, &type_fqn, lower)?;

    // 2) 主构造参数上的注解（包含 `@param:` / `@property:` / `@field:` 等 use-site target）。
    if let Some(primary_ctor) = &decl.primary_ctor {
        check_param_list_annotations(ctx, lower, builtins, &primary_ctor.params)?;
    }

    // 3) 递归检查类型体成员（包含 nested types）。
    let Some(body) = &decl.body else {
        return Ok(());
    };
    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                check_annotation_uses(
                    ctx,
                    lower,
                    builtins,
                    &v.annotations,
                    AnnotationSite::new(AnnotationTargetKind::EnumVariant),
                )?;
                reject_builtin_annotations_on_target(ctx.source, &v.annotations, "enum variant")?;
            }
            ast::TypeMember::Property(p) => {
                reject_annotation_modifier_on_non_type_target(&p.modifiers, "属性", p.name.span)?;
                check_annotation_uses(
                    ctx,
                    lower,
                    builtins,
                    &p.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Property),
                )?;
                reject_builtin_annotations_on_target(ctx.source, &p.annotations, "property")?;
            }
            ast::TypeMember::SecondaryCtor(ctor) => {
                reject_annotation_modifier_on_non_type_target(
                    &ctor.modifiers,
                    "构造器",
                    ctor.span,
                )?;
                check_annotation_uses(
                    ctx,
                    lower,
                    builtins,
                    &ctor.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Constructor),
                )?;
                reject_builtin_annotations_on_target(ctx.source, &ctor.annotations, "constructor")?;
                check_param_list_annotations(ctx, lower, builtins, &ctor.params)?;
            }
            ast::TypeMember::Fun(fun) => {
                reject_annotation_modifier_on_non_type_target(
                    &fun.modifiers,
                    "成员函数",
                    fun.name.span,
                )?;
                check_annotation_uses(
                    ctx,
                    lower,
                    builtins,
                    &fun.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Function),
                )?;
                check_builtin_annotations_on_fun_decl(ctx.source, fun, lower)?;
                check_param_list_annotations(ctx, lower, builtins, &fun.params)?;
            }
            ast::TypeMember::Type(nested) => {
                check_type_decl_annotations(ctx, lower, builtins, nested, &type_fqn)?;
            }
            ast::TypeMember::Object(obj) => {
                check_object_decl_annotations(ctx, lower, builtins, obj, &type_fqn)?;
            }
            ast::TypeMember::InitBlock(_b) => {}
        }
    }

    Ok(())
}

/// 检查一组参数上的注解使用（`@Name(...)`）。
fn check_param_list_annotations(
    ctx: AnnotationCheckContext<'_>,
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
        check_annotation_uses(ctx, lower, builtins, &p.annotations, site)?;
        reject_builtin_annotations_on_target(ctx.source, &p.annotations, "param")?;
    }
    Ok(())
}

/// 检查 object 声明上的注解，并递归检查其类型体成员（含 nested type/object）。
fn check_object_decl_annotations(
    ctx: AnnotationCheckContext<'_>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    obj: &ast::ObjectDecl,
    prefix: &str,
) -> Result<(), AnnotationError> {
    reject_annotation_modifier_on_non_type_target(
        &obj.modifiers,
        object_kind_name(obj.kind),
        obj.name.as_ref().map(|name| name.span).unwrap_or(obj.span),
    )?;

    // object 自身的注解使用。
    check_annotation_uses(
        ctx,
        lower,
        builtins,
        &obj.annotations,
        AnnotationSite::new(AnnotationTargetKind::Type),
    )?;
    check_builtin_annotations_on_object_decl(ctx.source, obj)?;

    let Some(body) = &obj.body else {
        return Ok(());
    };

    // 为递归处理 nested type/object 计算容器前缀（与 TypeEnv 的收集规则对齐）。
    let local_name = match &obj.name {
        Some(name) => ctx.source.slice(name.span).to_string(),
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
                    ctx,
                    lower,
                    builtins,
                    &v.annotations,
                    AnnotationSite::new(AnnotationTargetKind::EnumVariant),
                )?;
                reject_builtin_annotations_on_target(ctx.source, &v.annotations, "enum variant")?;
            }
            ast::TypeMember::Property(p) => {
                reject_annotation_modifier_on_non_type_target(&p.modifiers, "属性", p.name.span)?;
                check_annotation_uses(
                    ctx,
                    lower,
                    builtins,
                    &p.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Property),
                )?;
                reject_builtin_annotations_on_target(ctx.source, &p.annotations, "property")?;
            }
            ast::TypeMember::SecondaryCtor(ctor) => {
                reject_annotation_modifier_on_non_type_target(
                    &ctor.modifiers,
                    "构造器",
                    ctor.span,
                )?;
                check_annotation_uses(
                    ctx,
                    lower,
                    builtins,
                    &ctor.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Constructor),
                )?;
                reject_builtin_annotations_on_target(ctx.source, &ctor.annotations, "constructor")?;
            }
            ast::TypeMember::Fun(fun) => {
                reject_annotation_modifier_on_non_type_target(
                    &fun.modifiers,
                    "成员函数",
                    fun.name.span,
                )?;
                check_annotation_uses(
                    ctx,
                    lower,
                    builtins,
                    &fun.annotations,
                    AnnotationSite::new(AnnotationTargetKind::Function),
                )?;
                check_builtin_annotations_on_fun_decl(ctx.source, fun, lower)?;
            }
            ast::TypeMember::Type(nested) => {
                check_type_decl_annotations(ctx, lower, builtins, nested, &obj_fqn)?;
            }
            ast::TypeMember::Object(nested) => {
                check_object_decl_annotations(ctx, lower, builtins, nested, &obj_fqn)?;
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
    // spec §15.2：`annotation` 只服务于 `annotation class ...`，而不是一般 nominal type。
    if decl.kind != ast::TypeKind::Class {
        return Err(AnnotationError::AnnotationClassMustBeClass {
            type_fqn: type_fqn.to_string(),
            span: decl.name.span.into(),
        });
    }

    check_annotation_class_modifiers(decl, type_fqn)?;

    // compile-time marker 不引入 effect 参数。
    if let Some(eff_param) = &decl.eff_param {
        return Err(AnnotationError::AnnotationClassEffectParamNotSupported {
            type_fqn: type_fqn.to_string(),
            span: eff_param.span.into(),
        });
    }

    // compile-time marker 不引入 where 约束。
    if let Some(where_clause) = &decl.where_clause {
        return Err(AnnotationError::AnnotationClassWhereClauseNotSupported {
            type_fqn: type_fqn.to_string(),
            span: where_clause.span.into(),
        });
    }

    // compile-time marker 不引入泛型实例化面。
    if let Some(first) = decl.type_params.first() {
        let last = decl.type_params.last().unwrap_or(first);
        return Err(AnnotationError::AnnotationClassTypeParamsNotSupported {
            type_fqn: type_fqn.to_string(),
            span: Span::new(first.span.start, last.span.end).into(),
        });
    }

    // 当前阶段（T4012a）：annotation class 作为 data-only 容器，不允许 implements/extends。
    if let Some(st) = decl.supertypes.first() {
        return Err(AnnotationError::AnnotationClassSupertypesNotSupported {
            type_fqn: type_fqn.to_string(),
            span: st.span.into(),
        });
    }

    // 当前阶段（T4012a）：不支持类型体成员（方法/属性/init/secondary ctor 等）。
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

fn check_annotation_class_modifiers(
    decl: &ast::TypeDecl,
    type_fqn: &str,
) -> Result<(), AnnotationError> {
    for modifier in &decl.modifiers {
        match modifier {
            ast::Modifier::Public
            | ast::Modifier::Internal
            | ast::Modifier::Private
            | ast::Modifier::Annotation => {}
            other => {
                return Err(AnnotationError::AnnotationClassModifierNotSupported {
                    type_fqn: type_fqn.to_string(),
                    modifier: modifier_name(*other).to_string(),
                    span: decl.name.span.into(),
                });
            }
        }
    }
    Ok(())
}

fn reject_annotation_modifier_on_non_type_target(
    modifiers: &[ast::Modifier],
    found: &str,
    span: Span,
) -> Result<(), AnnotationError> {
    if modifiers.contains(&ast::Modifier::Annotation) {
        return Err(AnnotationError::AnnotationModifierInvalidTarget {
            found: found.to_string(),
            span: span.into(),
        });
    }
    Ok(())
}

fn modifier_name(modifier: ast::Modifier) -> &'static str {
    match modifier {
        ast::Modifier::Public => "public",
        ast::Modifier::Internal => "internal",
        ast::Modifier::Private => "private",
        ast::Modifier::Open => "open",
        ast::Modifier::Abstract => "abstract",
        ast::Modifier::Sealed => "sealed",
        ast::Modifier::Async => "async",
        ast::Modifier::Inline => "inline",
        ast::Modifier::Override => "override",
        ast::Modifier::Const => "const",
        ast::Modifier::Annotation => "annotation",
    }
}

fn object_kind_name(kind: ast::ObjectKind) -> &'static str {
    match kind {
        ast::ObjectKind::Object => "object",
        ast::ObjectKind::Companion => "companion object",
    }
}

/// 批量检查一组注解使用（`@Name(...)`）。
fn check_annotation_uses(
    ctx: AnnotationCheckContext<'_>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    annotations: &[ast::AnnotationUse],
    site: AnnotationSite,
) -> Result<(), AnnotationError> {
    for a in annotations {
        check_one_annotation_use(ctx, lower, builtins, a, site)?;
    }
    Ok(())
}

/// 检查单个注解使用：解析注解名并确认其引用一个注解类。
fn check_one_annotation_use(
    ctx: AnnotationCheckContext<'_>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    ann: &ast::AnnotationUse,
    site: AnnotationSite,
) -> Result<(), AnnotationError> {
    let (name, name_span) = annotation_name_and_span(ctx.source, ann);

    // T1003：内建注解（`@Unsafe/@NoGC/@Extern/@Intrinsic`）由编译器识别，
    // 不要求存在对应的 `annotation class` 声明。
    if builtin_annotation_kind(ctx.source, ann).is_some() {
        return Ok(());
    }

    // 复用 Index 的“按 package/import 规则解析类型名”的逻辑来解析注解类型。
    let ty = ast::TypeRef::Path(ast::TypePath {
        span: ann.span,
        segments: ann.path.clone(),
        args: Vec::new(),
    });

    let Some(fqn) = ctx.index.type_ref_to_fqn_in_file(ctx.source, ctx.file, &ty) else {
        return Err(AnnotationError::UnresolvedAnnotationType {
            name,
            span: name_span.into(),
        });
    };

    let Some(sym) = ctx.env.type_symbol(&fqn) else {
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

    let effective_target = effective_annotation_target(ctx.source, ann, site.primary_target);

    // T1016a：meta-annotations 的合法位置与最小参数检查。
    if fqn == "scoop.core.Target" {
        if !site.is_annotation_class_decl {
            return Err(AnnotationError::MetaAnnotationInvalidTarget {
                annotation: "@Target".to_string(),
                found: effective_target.as_str().to_string(),
                span: name_span.into(),
            });
        }
        check_target_annotation_args(ctx.source, ann)?;
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
        check_retention_annotation_args(ctx.source, ann)?;
        return Ok(());
    }

    // T1016a：若注解类声明了 `@Target(...)`，则在使用点强制执行目标限制。
    if let Some(allowed) = &sym.annotation_targets
        && !allowed.contains(&effective_target)
    {
        return Err(AnnotationError::AnnotationInvalidTarget {
            annotation: fqn,
            allowed: join_target_list(allowed),
            found: effective_target.as_str().to_string(),
            span: name_span.into(),
        });
    }

    // T1019：注解参数的“类型匹配 + 编译期常量”检查。
    check_annotation_args(ctx, lower, builtins, &fqn, sym, ann)?;

    Ok(())
}

fn check_target_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    for arg in &ann.args {
        let Some((variant_name, variant_span)) =
            extract_annotation_target_variant(source, &arg.value)
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
    ctx: AnnotationCheckContext<'_>,
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
                let name = name_id.text(ctx.source).to_string();
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
            ctx,
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
    ctx: AnnotationCheckContext<'_>,
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
        ast::ExprKind::Ident(id) => match ctx.source.slice(id.span) {
            "true" | "false" => Ok(builtins.bool_),
            _ => Err(not_const()),
        },
        ast::ExprKind::Unary {
            op, expr: inner, ..
        } => {
            let operand_ty = infer_annotation_const_expr_type(
                ctx,
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
                ctx,
                lower,
                builtins,
                lhs.as_ref(),
                annotation_fqn,
                param_name,
            )?;
            let rhs_ty = infer_annotation_const_expr_type(
                ctx,
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
                | ast::BinaryOp::BitOr => {
                    unify_integer_operands_for_const_expr(lhs, lhs_ty, rhs, rhs_ty, lower, builtins)
                        .ok_or_else(not_const)
                }

                ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                    if is_integer_type_for_const_expr(lhs_ty, lower, builtins)
                        && rhs_ty == builtins.int
                    {
                        Ok(lhs_ty)
                    } else {
                        Err(not_const())
                    }
                }

                ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                    if unify_integer_operands_for_const_expr(
                        lhs, lhs_ty, rhs, rhs_ty, lower, builtins,
                    )
                    .is_some()
                    {
                        Ok(builtins.bool_)
                    } else {
                        Err(not_const())
                    }
                }

                ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
                    if lhs_ty == builtins.bool_ && rhs_ty == builtins.bool_ {
                        return Ok(builtins.bool_);
                    }
                    if unify_integer_operands_for_const_expr(
                        lhs, lhs_ty, rhs, rhs_ty, lower, builtins,
                    )
                    .is_some()
                    {
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

                ast::BinaryOp::RangeInclusive => Err(not_const()),
                ast::BinaryOp::Elvis => Err(not_const()),
            }
        }
        ast::ExprKind::MemberAccess { .. } => infer_enum_unit_variant_const_type(
            ctx.source, ctx.file, ctx.index, ctx.env, lower, expr,
        )
        .ok_or_else(not_const),
        ast::ExprKind::ArrayLit { elements } => {
            let first = elements.first().ok_or_else(not_const)?;
            let mut elem_ty = infer_annotation_const_expr_type(
                ctx,
                lower,
                builtins,
                first,
                annotation_fqn,
                param_name,
            )?;

            for e in elements.iter().skip(1) {
                let ty = infer_annotation_const_expr_type(
                    ctx,
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
            if ctx
                .index
                .type_ref_to_fqn_in_file(ctx.source, ctx.file, ty)
                .is_none()
            {
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

fn is_integer_type_for_const_expr(
    ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
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

    if matches!(&lhs.kind, ast::ExprKind::IntLit)
        && is_integer_type_for_const_expr(rhs_ty, lower, builtins)
    {
        return Some(rhs_ty);
    }
    if matches!(&rhs.kind, ast::ExprKind::IntLit)
        && is_integer_type_for_const_expr(lhs_ty, lower, builtins)
    {
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

fn extract_annotation_target_variant(
    source: &SourceFile,
    expr: &ast::Expr,
) -> Option<(String, Span)> {
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

fn extract_string_literal_text(source: &SourceFile, expr: &ast::Expr) -> Option<String> {
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
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    let flags = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations);

    // 1) `@Unsafe/@NoGC/@Intrinsic` 当前不支持参数；`@Extern` 支持最小 FFI 形态参数（见 TODO T1020）。
    for ann in &fun.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        match kind {
            BuiltinAnnotationKind::Extern => check_extern_builtin_annotation_args(source, ann)?,
            BuiltinAnnotationKind::CallingConvention => {
                check_calling_convention_builtin_annotation_args(source, ann)?
            }
            BuiltinAnnotationKind::Unsafe
            | BuiltinAnnotationKind::Safe
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
    if flags.is_extern
        && let ast::FunBody::Block(b) = &fun.body
    {
        let fun_name = source.slice(fun.name.span).to_string();
        return Err(AnnotationError::ExternFunMustHaveNoBody {
            fun_name,
            span: b.span.into(),
        });
    }
    if flags.is_intrinsic
        && let ast::FunBody::Block(b) = &fun.body
    {
        let fun_name = source.slice(fun.name.span).to_string();
        return Err(AnnotationError::IntrinsicFunMustHaveNoBody {
            fun_name,
            span: b.span.into(),
        });
    }

    // 3) `@Extern`：C ABI 边界禁止直接透传 GC 引用类型（addrspace(1) ref 指针）。
    //
    // 说明：
    // - 这里采用保守判定：签名中的 receiver/参数/返回值都必须是 GC-free 值类型；
    // - 允许通过 `Ptr<T>` / `UIntPtr` 等显式桥接；`Ptr<T>` 的 pointee GC-free 门禁由 TypeLowering 负责。
    if flags.is_extern {
        check_extern_fun_signature_is_gc_free(source, fun, lower)?;
    }

    Ok(())
}

fn check_extern_fun_signature_is_gc_free(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    // receiver：`fun Receiver.name(...)` 语义上等价于第一个参数，同样属于 ABI 边界。
    if let Some(receiver) = fun.receiver.as_ref() {
        check_extern_abi_type_ref_is_gc_free(source, receiver, lower)?;
    }

    for p in &fun.params {
        let Some(ty_ref) = p.ty.as_ref() else {
            // 缺失类型由其它检查负责（保持健壮性）。
            continue;
        };
        check_extern_abi_type_ref_is_gc_free(source, ty_ref, lower)?;
    }

    // 缺省 return 为 Unit：天然 GC-free。
    if let Some(ret_ty_ref) = fun.return_ty.as_ref() {
        check_extern_abi_type_ref_is_gc_free(source, ret_ty_ref, lower)?;
    }

    Ok(())
}

fn check_extern_abi_type_ref_is_gc_free(
    _source: &SourceFile,
    ty_ref: &ast::TypeRef,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    let ty = match lower.lower_type_ref(ty_ref) {
        Ok(ty) => ty,
        Err(_e) => return Ok(()),
    };

    let is_gc_free = match lower.is_gc_free_value_type(ty) {
        Ok(v) => v,
        Err(_e) => return Ok(()),
    };

    if is_gc_free {
        return Ok(());
    }

    Err(AnnotationError::ExternFunSignatureMustBeGcFree {
        found: lower.fmt_type(ty),
        span: ty_ref.span().into(),
    })
}

fn check_builtin_annotations_on_type_alias_decl(
    source: &SourceFile,
    decl: &ast::TypeAliasDecl,
) -> Result<(), AnnotationError> {
    for ann in &decl.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };

        match kind {
            BuiltinAnnotationKind::CallingConvention => {
                check_calling_convention_builtin_annotation_args(source, ann)?;
            }
            _ => {
                let (_, name_span) = annotation_name_and_span(source, ann);
                return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                    annotation: format!("@{}", kind.name()),
                    allowed: kind.allowed_targets_hint(),
                    found: "typealias",
                    span: name_span.into(),
                });
            }
        }
    }
    Ok(())
}

fn check_builtin_annotations_on_top_level_val_decl(
    source: &SourceFile,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    let flags = BuiltinAnnotationFlags::from_annotations(source, &v.annotations);

    for ann in &v.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        if kind != BuiltinAnnotationKind::Extern {
            let (_, name_span) = annotation_name_and_span(source, ann);
            return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                annotation: format!("@{}", kind.name()),
                allowed: kind.allowed_targets_hint(),
                found: "val/var",
                span: name_span.into(),
            });
        }
        check_extern_builtin_annotation_args(source, ann)?;
    }

    if !flags.is_extern {
        return Ok(());
    }

    if let Some(init) = &v.init {
        let var_name = v
            .name()
            .map(|id| id.text(source).to_string())
            .unwrap_or_else(|| "<pattern>".to_string());
        return Err(AnnotationError::ExternVarInitializerNotAllowed {
            var_name,
            span: init.span.into(),
        });
    }

    let Some(ty_ref) = &v.ty else {
        // 顶层 val/var 的 `: Type` 缺失由 `check_file_headers` 处理；
        // 这里保持健壮性，不重复报错。
        return Ok(());
    };

    let ty = match lower.lower_type_ref(ty_ref) {
        Ok(ty) => ty,
        Err(_e) => return Ok(()),
    };

    let is_gc_free = match lower.is_gc_free_value_type(ty) {
        Ok(v) => v,
        Err(_e) => return Ok(()),
    };

    if !is_gc_free {
        return Err(AnnotationError::ExternVarTypeMustBeGcFree {
            found: lower.fmt_type(ty),
            span: ty_ref.span().into(),
        });
    }

    Ok(())
}

fn check_calling_convention_builtin_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    if ann.args.len() != 1 {
        return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
            span: ann.span.into(),
        });
    }

    let arg = &ann.args[0];
    let (key, key_span, value) = match &arg.name {
        Some(name_id) => (Some(name_id.text(source)), name_id.span, &arg.value),
        None => match &arg.value.kind {
            ast::ExprKind::Assign { lhs, rhs, .. } => {
                let ast::ExprKind::Ident(id) = &lhs.kind else {
                    return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
                        span: lhs.span.into(),
                    });
                };
                (Some(source.slice(id.span)), id.span, rhs.as_ref())
            }
            _ => (None, arg.span, &arg.value),
        },
    };

    if let Some(key) = key
        && key != "name"
    {
        return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
            span: key_span.into(),
        });
    }

    let Some(name) = extract_string_literal_text(source, value) else {
        return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
            span: value.span.into(),
        });
    };

    let normalized = name.trim().to_ascii_lowercase();
    if normalized != "c" && normalized != "cdecl" {
        return Err(AnnotationError::CallingConventionNotSupported {
            name,
            span: value.span.into(),
        });
    }

    Ok(())
}

fn check_top_level_var_storage_and_gc_free(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    if v.kind != ast::ValKind::Var {
        return Ok(());
    }

    // `@Extern var` 的语义由 T1020 处理：
    // - 不要求 `@ThreadLocal/@Global`（存储由外部系统提供）；
    // - GC-free 限制与 initializer 门禁由 `check_builtin_annotations_on_top_level_val_decl` 覆盖。
    let builtin_flags = BuiltinAnnotationFlags::from_annotations(source, &v.annotations);
    if builtin_flags.is_extern {
        return Ok(());
    }

    const THREAD_LOCAL_FQN: &str = "scoop.core.ThreadLocal";
    const GLOBAL_FQN: &str = "scoop.core.Global";

    let is_thread_local = v
        .annotations
        .iter()
        .any(|ann| annotation_use_resolves_to_fqn(source, file, index, ann, THREAD_LOCAL_FQN));
    let is_global = v
        .annotations
        .iter()
        .any(|ann| annotation_use_resolves_to_fqn(source, file, index, ann, GLOBAL_FQN));

    if !is_thread_local && !is_global {
        let (var_name, span) = match &v.binding {
            ast::ValBinding::Name(name) => (name.text(source).to_string(), name.span),
            ast::ValBinding::Pattern(_) => ("<pattern>".to_string(), v.span),
        };
        return Err(AnnotationError::TopLevelVarRequiresThreadLocalOrGlobal {
            var_name,
            span: span.into(),
        });
    }

    let Some(ty_ref) = &v.ty else {
        // 顶层 var 缺少 `: Type` 会在 headers check（T0404）中报错；
        // 这里保持健壮性，不重复报错。
        return Ok(());
    };

    let ty = match lower.lower_type_ref(ty_ref) {
        Ok(ty) => ty,
        Err(_e) => return Ok(()),
    };

    let is_gc_free = match lower.is_gc_free_value_type(ty) {
        Ok(v) => v,
        Err(_e) => return Ok(()),
    };

    if !is_gc_free {
        return Err(AnnotationError::TopLevelVarTypeMustBeGcFree {
            found: lower.fmt_type(ty),
            span: ty_ref.span().into(),
        });
    }

    Ok(())
}

/// 检查 `@CLayout(aligned, packed)` 在 struct 声明上的最小语义约束（spec §15.5.2）。
///
/// 当前阶段约束（为保证可单独回归、并避免过早引入复杂 ABI 规则）：
/// - 仅当注解解析到 `scoop.core.CLayout` 时才生效（避免同名注解误判）；
/// - struct 必须是 GC-free（直接/间接不含 GC 引用）；
/// - `packed`：仅支持 `packed = 1`（其它值给出稳定错误码）；
/// - `aligned`：必须是正的 2 的幂；`aligned = 0` 表示未指定（使用默认 ABI 对齐）。
fn check_clayout_struct_decl(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    decl: &ast::TypeDecl,
    type_fqn: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    if decl.kind != ast::TypeKind::Struct {
        return Ok(());
    }

    const CLAYOUT_FQN: &str = "scoop.core.CLayout";
    let Some(ann) = decl
        .annotations
        .iter()
        .find(|ann| annotation_use_resolves_to_fqn(source, file, index, ann, CLAYOUT_FQN))
    else {
        return Ok(());
    };

    // 1) GC-free 约束：`@CLayout` struct 不允许直接/间接包含 GC 引用。
    let ty = match lower.lower_type_fqn_with_args(type_fqn.to_string(), Vec::new(), decl.name.span)
    {
        Ok(ty) => ty,
        Err(_e) => {
            // 类型本身缺失/非法会在其它阶段给出更精确诊断；这里不重复报错。
            return Ok(());
        }
    };
    let is_gc_free = match lower.is_gc_free_value_type(ty) {
        Ok(v) => v,
        Err(_e) => return Ok(()),
    };
    if !is_gc_free {
        return Err(AnnotationError::CLayoutStructMustBeGcFree {
            struct_fqn: type_fqn.to_string(),
            span: ann.span.into(),
        });
    }

    // 2) 参数检查：只做最小解析与合法性约束；完整 ABI 行为由后端实现。
    let (aligned, aligned_span, packed, packed_span) = parse_clayout_args(source, ann)?;

    if let Some(value) = packed {
        // `packed` 必须是正的 2 的幂且 ≤ 16（等价于 C `#pragma pack(N)` 的合法值）。
        // 语义：每个字段的 alignment 取 `min(field_natural_align, N)`。
        if value == 0 || !value.is_power_of_two() || value > 16 {
            return Err(AnnotationError::CLayoutPackedValueNotSupported {
                value,
                span: packed_span.unwrap_or(ann.span).into(),
            });
        }
    }

    if let Some(value) = aligned
        && (value == 0 || !value.is_power_of_two())
    {
        return Err(AnnotationError::CLayoutAlignedValueInvalid {
            value,
            span: aligned_span.unwrap_or(ann.span).into(),
        });
    }

    Ok(())
}

/// 从 `@CLayout(...)` 注解实参中提取 `aligned`/`packed`。
///
/// 返回：
/// - `aligned`：`Some(N)` 表示显式指定；`None` 表示未指定或为 0；
/// - `packed`：`Some(M)` 表示显式指定；`None` 表示未指定或为 0；
/// - 同时返回各参数对应的 span（用于诊断定位）。
fn parse_clayout_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<ParsedCLayoutArgs, AnnotationError> {
    let mut aligned: Option<(u64, Span)> = None;
    let mut packed: Option<(u64, Span)> = None;

    for (pos, arg) in ann.args.iter().enumerate() {
        // 兼容三种参数形态：
        // - `aligned: 16`（AnnotationArg.name）
        // - `aligned = 16`（赋值表达式；更贴近 Kotlin 风格）
        // - 位置参数：`@CLayout(16, 1)`（按顺序映射到 aligned/packed）
        let (param, value) = match &arg.name {
            Some(name_id) => (Some(name_id.text(source)), Some(&arg.value)),
            None => match &arg.value.kind {
                ast::ExprKind::Assign { lhs, rhs, .. } => match &lhs.kind {
                    ast::ExprKind::Ident(id) => (Some(source.slice(id.span)), Some(rhs.as_ref())),
                    _ => (None, None),
                },
                _ => (None, Some(&arg.value)),
            },
        };
        let param = match param {
            Some(name) => name,
            None => match pos {
                0 => "aligned",
                1 => "packed",
                _ => continue,
            },
        };
        let Some(value) = value else { continue };

        let span = value.span;
        if !matches!(value.kind, ast::ExprKind::IntLit) {
            return Err(AnnotationError::CLayoutParamMustBeIntLiteral {
                param: param.to_string(),
                span: span.into(),
            });
        }

        let raw = source.slice(span);
        let value = parse_int_literal_u64(raw).unwrap_or(0);
        // `0` 视为“未指定”；其它值保留（由调用方进一步做合法性验证）。
        let value = if value == 0 { None } else { Some(value) };

        match param {
            "aligned" => {
                if let Some(v) = value {
                    aligned = Some((v, span));
                }
            }
            "packed" => {
                if let Some(v) = value {
                    packed = Some((v, span));
                }
            }
            _ => {}
        }
    }

    Ok((
        aligned.map(|(v, _)| v),
        aligned.map(|(_, s)| s),
        packed.map(|(v, _)| v),
        packed.map(|(_, s)| s),
    ))
}

fn parse_int_literal_u64(text: &str) -> Option<u64> {
    u64::try_from(parse_int_literal(text)).ok()
}

fn annotation_use_resolves_to_fqn(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ann: &ast::AnnotationUse,
    expected_fqn: &str,
) -> bool {
    // 复用 Index 的“按 package/import 规则解析类型名”的逻辑来解析注解类型名；
    // 这能避免仅按未限定名匹配导致的误判（同名但不同包的注解类）。
    let ty = ast::TypeRef::Path(ast::TypePath {
        span: ann.span,
        segments: ann.path.clone(),
        args: Vec::new(),
    });

    matches!(
        index.type_ref_to_fqn_in_file(source, file, &ty),
        Some(fqn) if fqn == expected_fqn
    )
}

fn check_extern_builtin_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    if ann.args.is_empty() {
        return Ok(());
    }

    let mut positional: Option<Span> = None;
    let mut name_arg: Option<Span> = None;
    let mut lib_arg: Option<Span> = None;
    let mut seen_named = false;

    for arg in &ann.args {
        // 允许两种“命名参数”写法：
        // - `lib: "m"`（AnnotationArg.name 形式；与普通注解参数一致）
        // - `lib = "m"`（赋值表达式；与 Kotlin 风格 `name = expr` 对齐）
        //
        // 注意：`@Extern` 是内建注解，不走通用注解参数绑定规则，因此这里单独解析。
        let (key, key_span, value) = match &arg.name {
            Some(name_id) => (name_id.text(source), name_id.span, &arg.value),
            None => match &arg.value.kind {
                ast::ExprKind::Assign { lhs, rhs, .. } => {
                    let ast::ExprKind::Ident(id) = &lhs.kind else {
                        return Err(AnnotationError::ExternAnnotationArgsInvalid {
                            span: lhs.span.into(),
                        });
                    };
                    (source.slice(id.span), id.span, rhs.as_ref())
                }
                _ => {
                    // 位置参数：仅允许单个字符串字面量（作为 symbol/name）。
                    if seen_named {
                        return Err(AnnotationError::ExternAnnotationArgsInvalid {
                            span: arg.span.into(),
                        });
                    }
                    if positional.is_some() {
                        return Err(AnnotationError::ExternAnnotationArgsInvalid {
                            span: arg.span.into(),
                        });
                    }
                    if !matches!(arg.value.kind, ast::ExprKind::StringLit) {
                        return Err(AnnotationError::ExternAnnotationArgsInvalid {
                            span: arg.value.span.into(),
                        });
                    }
                    positional = Some(arg.span);
                    continue;
                }
            },
        };

        seen_named = true;
        if !matches!(value.kind, ast::ExprKind::StringLit) {
            return Err(AnnotationError::ExternAnnotationArgsInvalid {
                span: value.span.into(),
            });
        }

        match key {
            "name" => {
                if name_arg.is_some() {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: key_span.into(),
                    });
                }
                name_arg = Some(key_span);
            }
            "lib" => {
                if lib_arg.is_some() {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: key_span.into(),
                    });
                }
                lib_arg = Some(key_span);
            }
            _ => {
                return Err(AnnotationError::ExternAnnotationArgsInvalid {
                    span: key_span.into(),
                });
            }
        }
    }

    // `@Extern("puts", name = "...")` 这类同时指定符号名的形态当前不支持（避免歧义）。
    if positional.is_some() && name_arg.is_some() {
        return Err(AnnotationError::ExternAnnotationArgsInvalid {
            span: ann.span.into(),
        });
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
        check_extern_builtin_annotation_args(source, ann)?;
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
