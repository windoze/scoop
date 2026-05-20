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
//! - 完整的 built-in annotation behavior（`@Deprecated/@Suppress/...`）；
//! - 注解在表达式位置的语义（如 `@Suppress(...) expr`）；
//! - 更丰富的 metaprogramming / reflection surface。

use std::collections::HashMap;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::hir::ExternAbi;
use crate::intrinsics::{
    IntrinsicAnnotationParseError, named_intrinsic_audit_entry, parse_intrinsic_annotation_args,
};
use crate::resolve::ImportTable;
use crate::resolve::Index;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::int_literal::parse_int_literal;
use crate::ty::{BuiltinTypes, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::assignable::is_type_assignable;
use super::builtin_annotations::{
    BuiltinAnnotationFlags, BuiltinAnnotationKind, builtin_annotation_kind, file_allows_intrinsic,
    parse_experimental_annotation, parse_suppress_annotation,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternAnnotationTarget {
    Function,
    NonFunction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternFunctionSite {
    TopLevel,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingRegularBodyPolicy {
    RequireBody,
    AllowAbstractDeclaration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ParsedExternAnnotationArgs {
    abi: ExternAbi,
    calling_convention_span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ParsedCallingConventionAnnotationArgs {
    convention_span: Option<Span>,
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

    #[error(
        "`@Extern` 仅支持：无参 / 单个字符串位置参数 / 命名参数 `name`、`lib`，以及函数声明上的 `abi`、`callingConvention`（字符串字面量）"
    )]
    #[diagnostic(code(scoop::typecheck::extern_annotation_args_invalid))]
    ExternAnnotationArgsInvalid {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 参数 `{param}` 重复指定")]
    #[diagnostic(code(scoop::typecheck::extern_annotation_arg_duplicate))]
    ExternAnnotationArgDuplicate {
        param: &'static str,
        #[label("这里重复指定")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的 `@Extern` ABI：{name}（当前仅支持 \"c\" / \"scoop\"）")]
    #[diagnostic(code(scoop::typecheck::extern_annotation_abi_not_supported))]
    ExternAnnotationAbiNotSupported {
        name: String,
        #[label("这里的 ABI 名称无效")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 的 `abi` 参数当前只支持函数声明")]
    #[diagnostic(code(scoop::typecheck::extern_annotation_abi_only_supported_on_functions))]
    ExternAnnotationAbiOnlySupportedOnFunctions {
        #[label("这里不能写 `abi = ...`")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 的 `callingConvention` 参数当前只支持函数声明")]
    #[diagnostic(code(
        scoop::typecheck::extern_annotation_calling_convention_only_supported_on_functions
    ))]
    ExternAnnotationCallingConventionOnlySupportedOnFunctions {
        #[label("这里不能写 `callingConvention = ...`")]
        span: miette::SourceSpan,
    },

    #[error(
        "`@CallingConvention` 仅支持：单个字符串位置参数，或命名参数 `convention` 与可选 `name`（字符串字面量）"
    )]
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

    #[error("`@CallingConvention` 函数必须提供函数体：{fun_name}")]
    #[diagnostic(code(scoop::typecheck::calling_convention_fun_must_have_body))]
    CallingConventionFunMustHaveBody {
        fun_name: String,
        #[label("这里需要函数体")]
        span: miette::SourceSpan,
    },

    #[error("`@CallingConvention` 当前不支持泛型函数")]
    #[diagnostic(code(scoop::typecheck::calling_convention_fun_generics_not_supported))]
    CallingConventionFunGenericsNotSupported {
        #[label("这里的类型参数不在当前支持范围内")]
        span: miette::SourceSpan,
    },

    #[error(
        "`@CallingConvention` 函数的 native ABI 签名只接受当前 native value surface：标量、`UIntPtr`、`Ptr<T>`、纯 `FunPtr<F>` token、tuple，以及 `@CLayout` struct；不接受 {found}"
    )]
    #[diagnostic(code(
        scoop::typecheck::calling_convention_fun_signature_not_supported_by_native_abi
    ))]
    CallingConventionFunSignatureNotSupportedByNativeAbi {
        found: String,
        #[label("这里的类型不在当前 native ABI contract 中")]
        span: miette::SourceSpan,
    },

    #[error("`@CallingConvention` 函数不允许声明非 Pure 的 effect row")]
    #[diagnostic(code(scoop::typecheck::calling_convention_fun_effects_not_allowed))]
    CallingConventionFunEffectsNotAllowed {
        #[label("native callable object symbol 必须是 Pure（或省略 effect row）")]
        span: miette::SourceSpan,
    },

    #[error("`@CallingConvention` 函数不允许声明 effect row 参数")]
    #[diagnostic(code(scoop::typecheck::calling_convention_fun_eff_param_not_allowed))]
    CallingConventionFunEffParamNotAllowed {
        #[label("native callable object symbol 不能依赖 effect 多态")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 函数必须省略函数体（外部实现）：{fun_name}")]
    #[diagnostic(code(scoop::typecheck::extern_fun_must_have_no_body))]
    ExternFunMustHaveNoBody {
        fun_name: String,
        #[label("这里不应有函数体")]
        span: miette::SourceSpan,
    },

    #[error(
        "普通{decl_kind}必须提供函数体（仅 `@Intrinsic`、`@Extern` 或无默认实现的 interface method 可省略）：{fun_name}"
    )]
    #[diagnostic(code(scoop::typecheck::fun_must_have_body))]
    FunMustHaveBody {
        decl_kind: &'static str,
        fun_name: String,
        #[label("这里需要函数体")]
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
        "`@Extern` 函数的 native ABI 签名只接受当前 native value surface：标量、`UIntPtr`、`Ptr<T>`、纯 `FunPtr<F>` token、tuple，以及 `@CLayout` struct；不接受 {found}；长期 opaque token 请 round-trip `GcHandle.raw: UIntPtr`，短时裸地址借出请使用 `GC.pin/unpin` + `scoop.unsafe.Ptr<T>`"
    )]
    #[diagnostic(code(scoop::typecheck::extern_fun_signature_not_supported_by_native_abi))]
    ExternFunSignatureNotSupportedByNativeAbi {
        found: String,
        #[label("这里的类型不在当前 native ABI contract 中")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 函数不允许声明非 Pure 的 effect row")]
    #[diagnostic(code(scoop::typecheck::extern_fun_effects_not_allowed))]
    ExternFunEffectsNotAllowed {
        #[label("普通 `@Extern` ABI 必须是 Pure（或省略 effect row）")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 函数不允许声明 effect row 参数")]
    #[diagnostic(code(scoop::typecheck::extern_fun_eff_param_not_allowed))]
    ExternFunEffParamNotAllowed {
        #[label("普通 `@Extern` ABI 不能依赖 effect 多态")]
        span: miette::SourceSpan,
    },

    #[error(
        "`abi = \"scoop\"` 当前不支持 `callingConvention`；Managed ABI 不是 machine calling convention 扩展点"
    )]
    #[diagnostic(code(scoop::typecheck::extern_fun_scoop_abi_calling_convention_not_supported))]
    ExternFunScoopAbiCallingConventionNotSupported {
        #[label("这里的 `callingConvention` 对 `abi = \"scoop\"` 无效")]
        span: miette::SourceSpan,
    },

    #[error(
        "`@Extern` 函数不再支持单独叠加 `@CallingConvention`；请改用 `@Extern(..., callingConvention = \"...\")`"
    )]
    #[diagnostic(code(scoop::typecheck::extern_fun_calling_convention_annotation_not_allowed))]
    ExternFunCallingConventionAnnotationNotAllowed {
        #[label("这里的 calling convention 必须写在 `@Extern` 参数中")]
        span: miette::SourceSpan,
    },

    #[error("`abi = \"c\"` 的 `@Extern` 已隐含 `{annotation}`，不允许重复标注")]
    #[diagnostic(code(scoop::typecheck::extern_fun_c_abi_modifier_redundant))]
    ExternFunCAbiModifierRedundant {
        annotation: String,
        #[label("这里的注解语义已由 C ABI 隐含")]
        span: miette::SourceSpan,
    },

    #[error("`abi = \"scoop\"` 的 `@Extern` 不支持 `{annotation}`")]
    #[diagnostic(code(scoop::typecheck::extern_fun_scoop_abi_modifier_not_supported))]
    ExternFunScoopAbiModifierNotSupported {
        annotation: String,
        #[label("这里的注解与 Managed ABI 语义冲突")]
        span: miette::SourceSpan,
    },

    #[error("`abi = \"scoop\"` 当前只支持无 receiver 的顶层函数")]
    #[diagnostic(code(scoop::typecheck::extern_fun_scoop_abi_requires_top_level_fun))]
    ExternFunScoopAbiRequiresTopLevelFun {
        #[label("这里不在当前 v1 支持范围内")]
        span: miette::SourceSpan,
    },

    #[error("`abi = \"scoop\"` 当前不支持泛型函数")]
    #[diagnostic(code(scoop::typecheck::extern_fun_scoop_abi_generics_not_supported))]
    ExternFunScoopAbiGenericsNotSupported {
        #[label("这里的类型参数不在当前 v1 支持范围内")]
        span: miette::SourceSpan,
    },

    #[error("`abi = \"scoop\"` v1 暂不支持 function value / continuation 跨边界：{found}")]
    #[diagnostic(code(scoop::typecheck::extern_fun_scoop_abi_callable_surface_not_supported))]
    ExternFunScoopAbiCallableSurfaceNotSupported {
        found: String,
        #[label("这里的类型仍不在当前 v1 支持范围内")]
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

    #[error(
        "函数上的 `@Intrinsic` 只支持零参数或单个字符串位置参数：`@Intrinsic` / `@Intrinsic(\"dummy_ir\")`"
    )]
    #[diagnostic(code(scoop::typecheck::intrinsic_annotation_invalid_arg_shape))]
    IntrinsicAnnotationInvalidArgShape {
        #[label("这里的 `@Intrinsic` 参数形状不受支持")]
        span: miette::SourceSpan,
    },

    #[error("`@Intrinsic(\"name\")` 的参数必须是字符串字面量")]
    #[diagnostic(code(scoop::typecheck::intrinsic_annotation_arg_must_be_string))]
    IntrinsicAnnotationArgMustBeString {
        #[label("这里需要字符串字面量")]
        span: miette::SourceSpan,
    },

    #[error("`@Intrinsic(\"{name}\")` 未命中编译器 intrinsic 表")]
    #[diagnostic(code(scoop::typecheck::unknown_intrinsic_table_entry))]
    UnknownIntrinsicTableEntry {
        name: String,
        #[label("这里的 intrinsic name 不在编译器表中")]
        span: miette::SourceSpan,
    },

    #[error("`@Intrinsic` 类型不能声明字段：{type_fqn}.{field_name}（layout 仍由编译器内置）")]
    #[diagnostic(code(scoop::typecheck::intrinsic_type_field_not_supported))]
    IntrinsicTypeFieldNotSupported {
        type_fqn: String,
        field_name: String,
        #[label("这里声明了字段")]
        span: miette::SourceSpan,
    },

    #[error("{decl_kind} `{decl_name}` 只能在 trusted `syslib` cone 中声明 `@Intrinsic`")]
    #[diagnostic(
        code(scoop::typecheck::intrinsic_decl_requires_trusted_syslib),
        help("`@Intrinsic` 是 compiler surface 特权，不能由用户源码或普通 `lib` cone 自行开启")
    )]
    IntrinsicDeclRequiresTrustedSyslib {
        decl_kind: &'static str,
        decl_name: String,
        #[label("这里需要 trusted `syslib` 身份")]
        span: miette::SourceSpan,
    },

    #[error("`@file:AllowIntrinsic` 只能在 trusted `syslib` cone 中使用")]
    #[diagnostic(
        code(scoop::typecheck::allow_intrinsic_requires_trusted_syslib),
        help("普通用户源码和普通 `lib` cone 不能通过文件级注解获得 intrinsic 声明权限")
    )]
    AllowIntrinsicRequiresTrustedSyslib {
        #[label("这里需要 trusted `syslib` 身份")]
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

    #[error(
        "内建注解 `@Deprecated` 只有第一个参数允许使用位置参数；后续参数必须使用命名形式 `replaceWith: ...`"
    )]
    #[diagnostic(code(scoop::typecheck::deprecated_annotation_only_first_arg_positional))]
    DeprecatedAnnotationOnlyFirstArgPositional {
        #[label("这里需要改为命名参数 `replaceWith: ...`")]
        span: miette::SourceSpan,
    },

    #[error("`@Suppress` 至少需要一个 warning code（例如 `@Suppress(\"deprecated\")`）")]
    #[diagnostic(code(scoop::typecheck::suppress_annotation_requires_warning_codes))]
    SuppressAnnotationRequiresWarningCodes {
        #[label("这里缺少 warning code")]
        span: miette::SourceSpan,
    },

    #[error(
        "`@Suppress` 只支持位置字符串参数：`@Suppress(\"deprecated\", \"enum-size-disparity\")`"
    )]
    #[diagnostic(code(scoop::typecheck::suppress_annotation_named_args_not_supported))]
    SuppressAnnotationNamedArgsNotSupported {
        #[label("这里不能写命名参数")]
        span: miette::SourceSpan,
    },

    #[error("`@Suppress` 的参数必须是 warning code 字符串字面量")]
    #[diagnostic(code(scoop::typecheck::suppress_annotation_arg_must_be_string))]
    SuppressAnnotationArgMustBeString {
        #[label("这里需要字符串字面量")]
        span: miette::SourceSpan,
    },

    #[error("未知的 warning code：{code}")]
    #[diagnostic(code(scoop::typecheck::unknown_suppress_warning_code))]
    UnknownSuppressWarningCode {
        code: String,
        #[label("这里的 warning code 不受支持")]
        span: miette::SourceSpan,
    },

    #[error("`@Experimental` 只支持固定形状 `@Experimental(feature = \"some_feature\")`")]
    #[diagnostic(code(scoop::typecheck::experimental_annotation_invalid_arg_shape))]
    ExperimentalAnnotationInvalidArgShape {
        #[label("这里需要写成 `feature = \"...\"`")]
        span: miette::SourceSpan,
    },

    #[error("`@Experimental` 的 `feature` 参数必须是字符串字面量")]
    #[diagnostic(code(scoop::typecheck::experimental_annotation_arg_must_be_string))]
    ExperimentalAnnotationArgMustBeString {
        #[label("这里需要字符串字面量")]
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
    check_builtin_annotations_on_file(source, &file.file_annotations)?;
    let file_allows_intrinsic = file_allows_intrinsic(source, &file.file_annotations);

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
                check_builtin_annotations_on_fun_decl(
                    source,
                    file_allows_intrinsic,
                    fun,
                    &mut lower,
                    ExternFunctionSite::TopLevel,
                    MissingRegularBodyPolicy::RequireBody,
                )?;
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
                reject_builtin_annotations_on_target(
                    source,
                    &p.annotations,
                    AnnotationTargetKind::Property,
                    "extension property",
                )?;
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
                check_type_decl_annotations(
                    ctx,
                    &mut lower,
                    builtins,
                    ty,
                    &pkg_prefix,
                    file_allows_intrinsic,
                )?;
            }
            ast::Item::Object(obj) => {
                check_object_decl_annotations(
                    ctx,
                    &mut lower,
                    builtins,
                    obj,
                    &pkg_prefix,
                    file_allows_intrinsic,
                )?;
            }
            // T1220a：package-level comptime if 在进入 typecheck 之前应被裁剪（TODO T1220b）。
            ast::Item::ComptimeIf(_ci) => {}
        }
    }

    Ok(())
}

pub(crate) fn check_inline_annotation_uses(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
    primary_target: AnnotationTargetKind,
) -> Result<(), AnnotationError> {
    let site = AnnotationSite::new(primary_target);
    for ann in annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        match kind {
            BuiltinAnnotationKind::Suppress => {
                check_builtin_suppress_annotation(source, ann, site)?
            }
            _ => {
                let (_, name_span) = annotation_name_and_span(source, ann);
                return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                    annotation: format!("@{}", kind.name()),
                    allowed: kind.allowed_targets_hint(),
                    found: primary_target.as_str(),
                    span: name_span.into(),
                });
            }
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
    file_allows_intrinsic: bool,
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
    check_builtin_annotations_on_type_decl(ctx.source, file_allows_intrinsic, decl, &type_fqn)?;

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
                reject_builtin_annotations_on_target(
                    ctx.source,
                    &v.annotations,
                    AnnotationTargetKind::EnumVariant,
                    "enum variant",
                )?;
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
                reject_builtin_annotations_on_target(
                    ctx.source,
                    &p.annotations,
                    AnnotationTargetKind::Property,
                    "property",
                )?;
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
                reject_builtin_annotations_on_target(
                    ctx.source,
                    &ctor.annotations,
                    AnnotationTargetKind::Constructor,
                    "constructor",
                )?;
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
                check_builtin_annotations_on_fun_decl(
                    ctx.source,
                    file_allows_intrinsic,
                    fun,
                    lower,
                    ExternFunctionSite::Member,
                    missing_regular_body_policy_for_type(decl.kind),
                )?;
                check_param_list_annotations(ctx, lower, builtins, &fun.params)?;
            }
            ast::TypeMember::Type(nested) => {
                check_type_decl_annotations(
                    ctx,
                    lower,
                    builtins,
                    nested,
                    &type_fqn,
                    file_allows_intrinsic,
                )?;
            }
            ast::TypeMember::Object(obj) => {
                check_object_decl_annotations(
                    ctx,
                    lower,
                    builtins,
                    obj,
                    &type_fqn,
                    file_allows_intrinsic,
                )?;
            }
            ast::TypeMember::InitBlock(_b) => {}
        }
    }

    Ok(())
}

fn missing_regular_body_policy_for_type(kind: ast::TypeKind) -> MissingRegularBodyPolicy {
    match kind {
        ast::TypeKind::Interface | ast::TypeKind::Effect => {
            MissingRegularBodyPolicy::AllowAbstractDeclaration
        }
        ast::TypeKind::Class | ast::TypeKind::Struct | ast::TypeKind::Enum => {
            MissingRegularBodyPolicy::RequireBody
        }
    }
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
        reject_builtin_annotations_on_target(
            ctx.source,
            &p.annotations,
            site.primary_target,
            "param",
        )?;
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
    file_allows_intrinsic: bool,
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
    check_builtin_annotations_on_object_decl(ctx.source, file_allows_intrinsic, obj, &obj_fqn)?;

    let Some(body) = &obj.body else {
        return Ok(());
    };

    // 为递归处理 nested type/object 计算容器前缀（与 TypeEnv 的收集规则对齐）。

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
                reject_builtin_annotations_on_target(
                    ctx.source,
                    &v.annotations,
                    AnnotationTargetKind::EnumVariant,
                    "enum variant",
                )?;
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
                reject_builtin_annotations_on_target(
                    ctx.source,
                    &p.annotations,
                    AnnotationTargetKind::Property,
                    "property",
                )?;
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
                reject_builtin_annotations_on_target(
                    ctx.source,
                    &ctor.annotations,
                    AnnotationTargetKind::Constructor,
                    "constructor",
                )?;
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
                check_builtin_annotations_on_fun_decl(
                    ctx.source,
                    file_allows_intrinsic,
                    fun,
                    lower,
                    ExternFunctionSite::Member,
                    MissingRegularBodyPolicy::RequireBody,
                )?;
            }
            ast::TypeMember::Type(nested) => {
                check_type_decl_annotations(
                    ctx,
                    lower,
                    builtins,
                    nested,
                    &obj_fqn,
                    file_allows_intrinsic,
                )?;
            }
            ast::TypeMember::Object(nested) => {
                check_object_decl_annotations(
                    ctx,
                    lower,
                    builtins,
                    nested,
                    &obj_fqn,
                    file_allows_intrinsic,
                )?;
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
    if let Some(kind) = builtin_annotation_kind(ctx.source, ann) {
        return match kind {
            BuiltinAnnotationKind::Inline => check_builtin_inline_annotation(ctx.source, ann, site),
            BuiltinAnnotationKind::Deprecated => {
                check_builtin_deprecated_annotation(ctx, lower, builtins, ann, site)
            }
            BuiltinAnnotationKind::Suppress => {
                check_builtin_suppress_annotation(ctx.source, ann, site)
            }
            BuiltinAnnotationKind::Experimental => {
                check_builtin_experimental_annotation(ctx.source, ann, site)
            }
            _ => Ok(()),
        };
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

fn check_builtin_deprecated_annotation(
    ctx: AnnotationCheckContext<'_>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    ann: &ast::AnnotationUse,
    site: AnnotationSite,
) -> Result<(), AnnotationError> {
    let effective_target = effective_annotation_target(ctx.source, ann, site.primary_target);
    if !matches!(
        effective_target,
        AnnotationTargetKind::Function
            | AnnotationTargetKind::Type
            | AnnotationTargetKind::Property
    ) {
        let (_, name_span) = annotation_name_and_span(ctx.source, ann);
        return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
            annotation: "@Deprecated".to_string(),
            allowed: BuiltinAnnotationKind::Deprecated.allowed_targets_hint(),
            found: effective_target.as_str(),
            span: name_span.into(),
        });
    }

    check_builtin_deprecated_arg_surface(ctx.source, ann)?;

    let Some(sym) = ctx.env.type_symbol("scoop.core.Deprecated") else {
        return Err(AnnotationError::UnresolvedAnnotationType {
            name: "Deprecated".to_string(),
            span: ann.span.into(),
        });
    };
    check_annotation_args(ctx, lower, builtins, "scoop.core.Deprecated", sym, ann)
}

fn check_builtin_inline_annotation(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
    site: AnnotationSite,
) -> Result<(), AnnotationError> {
    let effective_target = effective_annotation_target(source, ann, site.primary_target);
    if !matches!(effective_target, AnnotationTargetKind::Function) {
        let (_, name_span) = annotation_name_and_span(source, ann);
        return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
            annotation: "@Inline".to_string(),
            allowed: BuiltinAnnotationKind::Inline.allowed_targets_hint(),
            found: effective_target.as_str(),
            span: name_span.into(),
        });
    }

    if !ann.args.is_empty() {
        let (_, name_span) = annotation_name_and_span(source, ann);
        return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
            annotation: "@Inline".to_string(),
            span: name_span.into(),
        });
    }

    Ok(())
}

fn check_builtin_deprecated_arg_surface(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    let mut seen_named = false;
    let mut positional_count = 0usize;

    for arg in &ann.args {
        if arg.name.is_some() {
            seen_named = true;
            continue;
        }
        if seen_named {
            return Err(AnnotationError::AnnotationArgPositionalAfterNamed {
                annotation: "scoop.core.Deprecated".to_string(),
                span: arg.span.into(),
            });
        }
        positional_count += 1;
        if positional_count > 1 {
            return Err(
                AnnotationError::DeprecatedAnnotationOnlyFirstArgPositional {
                    span: arg.span.into(),
                },
            );
        }
    }

    let _ = source;
    Ok(())
}

fn check_builtin_suppress_annotation(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
    site: AnnotationSite,
) -> Result<(), AnnotationError> {
    let effective_target = effective_annotation_target(source, ann, site.primary_target);
    if !matches!(
        effective_target,
        AnnotationTargetKind::Function
            | AnnotationTargetKind::Property
            | AnnotationTargetKind::Field
            | AnnotationTargetKind::Param
            | AnnotationTargetKind::Type
            | AnnotationTargetKind::Constructor
            | AnnotationTargetKind::LocalVariable
            | AnnotationTargetKind::Expression
            | AnnotationTargetKind::Module
            | AnnotationTargetKind::TypeParam
            | AnnotationTargetKind::EnumVariant
    ) {
        let (_, name_span) = annotation_name_and_span(source, ann);
        return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
            annotation: "@Suppress".to_string(),
            allowed: BuiltinAnnotationKind::Suppress.allowed_targets_hint(),
            found: effective_target.as_str(),
            span: name_span.into(),
        });
    }

    parse_suppress_annotation(source, ann)
        .map(|_| ())
        .map_err(|err| match err {
            super::builtin_annotations::SuppressAnnotationParseError::MissingWarningCodes {
                span,
            } => AnnotationError::SuppressAnnotationRequiresWarningCodes { span: span.into() },
            super::builtin_annotations::SuppressAnnotationParseError::NamedArgsNotSupported {
                span,
            } => AnnotationError::SuppressAnnotationNamedArgsNotSupported { span: span.into() },
            super::builtin_annotations::SuppressAnnotationParseError::ArgMustBeString { span } => {
                AnnotationError::SuppressAnnotationArgMustBeString { span: span.into() }
            }
            super::builtin_annotations::SuppressAnnotationParseError::UnknownWarningCode {
                code,
                span,
            } => AnnotationError::UnknownSuppressWarningCode {
                code,
                span: span.into(),
            },
        })
}

fn check_builtin_experimental_annotation(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
    site: AnnotationSite,
) -> Result<(), AnnotationError> {
    let effective_target = effective_annotation_target(source, ann, site.primary_target);
    if !matches!(
        effective_target,
        AnnotationTargetKind::Function
            | AnnotationTargetKind::Type
            | AnnotationTargetKind::Property
            | AnnotationTargetKind::Module
    ) {
        let (_, name_span) = annotation_name_and_span(source, ann);
        return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
            annotation: "@Experimental".to_string(),
            allowed: BuiltinAnnotationKind::Experimental.allowed_targets_hint(),
            found: effective_target.as_str(),
            span: name_span.into(),
        });
    }

    parse_experimental_annotation(source, ann)
        .map(|_| ())
        .map_err(|err| match err {
            super::builtin_annotations::ExperimentalAnnotationParseError::MissingFeature {
                span,
            } => AnnotationError::AnnotationArgMissingRequired {
                annotation: "@Experimental".to_string(),
                param: "feature".to_string(),
                span: span.into(),
            },
            super::builtin_annotations::ExperimentalAnnotationParseError::InvalidArgShape {
                span,
            } => AnnotationError::ExperimentalAnnotationInvalidArgShape { span: span.into() },
            super::builtin_annotations::ExperimentalAnnotationParseError::DuplicateFeature {
                span,
            } => AnnotationError::AnnotationArgDuplicate {
                annotation: "@Experimental".to_string(),
                param: "feature".to_string(),
                span: span.into(),
            },
            super::builtin_annotations::ExperimentalAnnotationParseError::ArgMustBeString {
                span,
            } => AnnotationError::ExperimentalAnnotationArgMustBeString { span: span.into() },
        })
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

        let expected_ty = match lower.with_annotation_types_allowed(|lower| {
            lower.lower_type_ref_in_decl_file(&sym.decl_file, &params[idx].ty)
        }) {
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
    primary_target: AnnotationTargetKind,
    found: &'static str,
) -> Result<(), AnnotationError> {
    for ann in annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        if kind == BuiltinAnnotationKind::Suppress {
            continue;
        }
        if kind == BuiltinAnnotationKind::Deprecated
            && matches!(
                effective_annotation_target(source, ann, primary_target),
                AnnotationTargetKind::Function
                    | AnnotationTargetKind::Type
                    | AnnotationTargetKind::Property
            )
        {
            continue;
        }
        if kind == BuiltinAnnotationKind::Experimental
            && matches!(
                effective_annotation_target(source, ann, primary_target),
                AnnotationTargetKind::Function
                    | AnnotationTargetKind::Type
                    | AnnotationTargetKind::Property
                    | AnnotationTargetKind::Module
            )
        {
            continue;
        }
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

fn check_builtin_annotations_on_file(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
) -> Result<(), AnnotationError> {
    for ann in annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        match kind {
            BuiltinAnnotationKind::AllowIntrinsic => {
                let (_, name_span) = annotation_name_and_span(source, ann);
                if !source.is_trusted_syslib() {
                    return Err(AnnotationError::AllowIntrinsicRequiresTrustedSyslib {
                        span: name_span.into(),
                    });
                }
                if !ann.args.is_empty() {
                    return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
                        annotation: format!("@{}", kind.name()),
                        span: name_span.into(),
                    });
                }
            }
            BuiltinAnnotationKind::Deprecated
            | BuiltinAnnotationKind::Suppress
            | BuiltinAnnotationKind::Experimental => {}
            _ => {
                let (_, name_span) = annotation_name_and_span(source, ann);
                return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                    annotation: format!("@{}", kind.name()),
                    allowed: kind.allowed_targets_hint(),
                    found: "file",
                    span: name_span.into(),
                });
            }
        }
    }
    Ok(())
}

fn check_builtin_annotations_on_fun_decl(
    source: &SourceFile,
    file_allows_intrinsic: bool,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
    fun_site: ExternFunctionSite,
    missing_body_policy: MissingRegularBodyPolicy,
) -> Result<(), AnnotationError> {
    let flags = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations);
    let fun_name = source.slice(fun.name.span).to_string();
    let mut extern_args = None;
    let mut calling_convention_args = None;
    let mut calling_convention_annotation_span = None;

    // 1) `@Unsafe/@NoGC` 当前不支持参数；`@Extern` 支持最小 FFI 形态参数；
    //    `@Intrinsic` 继续支持 legacy 零参数形态，并新增 `@Intrinsic("name")`。
    for ann in &fun.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        match kind {
            BuiltinAnnotationKind::Extern => {
                extern_args = Some(check_extern_builtin_annotation_args(
                    source,
                    ann,
                    ExternAnnotationTarget::Function,
                )?);
            }
            BuiltinAnnotationKind::CallingConvention => {
                let args = check_calling_convention_builtin_annotation_args(source, ann)?;
                let (_, name_span) = annotation_name_and_span(source, ann);
                calling_convention_args = Some(args);
                calling_convention_annotation_span = Some(name_span);
            }
            BuiltinAnnotationKind::AllowIntrinsic => {
                let (_, name_span) = annotation_name_and_span(source, ann);
                return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                    annotation: format!("@{}", kind.name()),
                    allowed: kind.allowed_targets_hint(),
                    found: "function",
                    span: name_span.into(),
                });
            }
            BuiltinAnnotationKind::Intrinsic => {
                check_intrinsic_builtin_annotation_gate(
                    source,
                    file_allows_intrinsic,
                    ann,
                    "函数",
                    &fun_name,
                )?;
                check_intrinsic_builtin_annotation_args(source, ann)?;
            }
            BuiltinAnnotationKind::Unsafe
            | BuiltinAnnotationKind::Safe
            | BuiltinAnnotationKind::NoGC
            | BuiltinAnnotationKind::Inline => {
                if !ann.args.is_empty() {
                    let (_, name_span) = annotation_name_and_span(source, ann);
                    return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
                        annotation: format!("@{}", kind.name()),
                        span: name_span.into(),
                    });
                }
            }
            BuiltinAnnotationKind::Deprecated
            | BuiltinAnnotationKind::Suppress
            | BuiltinAnnotationKind::Experimental => {}
        }
    }

    // 2) `@Extern/@Intrinsic`：声明必须省略函数体（实现由外部/编译器提供）。
    if flags.is_extern
        && let ast::FunBody::Block(b) = &fun.body
    {
        return Err(AnnotationError::ExternFunMustHaveNoBody {
            fun_name,
            span: b.span.into(),
        });
    }
    if flags.is_intrinsic
        && let ast::FunBody::Block(b) = &fun.body
    {
        return Err(AnnotationError::IntrinsicFunMustHaveNoBody {
            fun_name,
            span: b.span.into(),
        });
    }

    if regular_fun_requires_body(fun, &flags, missing_body_policy) {
        return Err(AnnotationError::FunMustHaveBody {
            decl_kind: missing_body_decl_kind(fun, fun_site),
            fun_name,
            span: fun.name.span.into(),
        });
    }

    if flags.is_extern {
        if let Some(span) = calling_convention_annotation_span {
            return Err(
                AnnotationError::ExternFunCallingConventionAnnotationNotAllowed {
                    span: span.into(),
                },
            );
        }

        let extern_args = extern_args.unwrap_or_default();
        let extern_abi = extern_args.abi;

        // 3) `@Extern`：`@Unsafe/@NoGC` 由 ABI 决定，不能再显式叠加。
        check_extern_fun_modifier_contract(source, &fun.annotations, extern_abi)?;

        // 4) `@Extern`：当前 v1 边界必须是 effect-impermeable。
        //
        // 说明：
        // - `ExternAbi::C` 调用只负责“进入 native -> 返回普通 ABI 结果”；
        // - `ExternAbi::Scoop` v1 仍未定义 outward effect / continuation ABI；
        // - outward/inward effect、continuation、non-local control 都不得通过该边界传播；
        // - 返回 `FunPtr<F>` / `UIntPtr` / stable handle 等 token 也不放宽上述规则；这些值只是
        //   原始地址/身份 token，而不是 effect/control bridge。
        check_extern_fun_effect_contract(fun)?;

        match extern_abi {
            ExternAbi::C => {
                // `@Extern`：native callable surface 必须走当前发布的 value contract。
                //
                // 说明：
                // - `@Extern` 与 native `FunPtr` 共享同一份 front-end gate；
                // - 允许通过 `Ptr<T>` / `UIntPtr` / `FunPtr<F>` token、tuple、`@CLayout` struct 等 current v1 surface 过边界；
                // - GC-managed ref / ordinary nominal aggregate 等不再继续通过“GC-free 近似”被默认放行。
                check_extern_fun_signature_matches_native_abi(source, fun, lower)?;
            }
            ExternAbi::Scoop => {
                if let Some(span) = extern_args.calling_convention_span {
                    return Err(
                        AnnotationError::ExternFunScoopAbiCallingConventionNotSupported {
                            span: span.into(),
                        },
                    );
                }
                check_extern_fun_scoop_v1_decl_shape(fun, fun_site)?;
                check_extern_fun_signature_matches_scoop_abi_v1(source, fun, lower)?;
            }
        }
    } else if let Some(args) = calling_convention_args {
        check_calling_convention_fun_contract(source, fun, lower, args)?;
    }

    Ok(())
}

fn regular_fun_requires_body(
    fun: &ast::FunDecl,
    flags: &BuiltinAnnotationFlags,
    missing_body_policy: MissingRegularBodyPolicy,
) -> bool {
    matches!(fun.body, ast::FunBody::Missing)
        && fun.kind == ast::FunDeclKind::Regular
        && missing_body_policy == MissingRegularBodyPolicy::RequireBody
        && !flags.is_extern
        && !flags.is_intrinsic
}

fn missing_body_decl_kind(fun: &ast::FunDecl, fun_site: ExternFunctionSite) -> &'static str {
    match (fun_site, fun.receiver.is_some()) {
        (ExternFunctionSite::TopLevel, true) => "扩展函数",
        (ExternFunctionSite::TopLevel, false) => "顶层函数",
        (ExternFunctionSite::Member, _) => "成员函数",
    }
}

fn check_intrinsic_builtin_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    let parsed = parse_intrinsic_annotation_args(source, ann).map_err(|error| match error {
        IntrinsicAnnotationParseError::InvalidShape { span } => {
            AnnotationError::IntrinsicAnnotationInvalidArgShape { span: span.into() }
        }
        IntrinsicAnnotationParseError::ArgMustBeString { span } => {
            AnnotationError::IntrinsicAnnotationArgMustBeString { span: span.into() }
        }
    })?;
    let Some(entry_name) = parsed.entry_name() else {
        return Ok(());
    };
    let entry_span = ann
        .args
        .first()
        .map(|arg| arg.value.span)
        .unwrap_or(ann.span);
    if named_intrinsic_audit_entry(entry_name).is_none() {
        return Err(AnnotationError::UnknownIntrinsicTableEntry {
            name: entry_name.to_string(),
            span: entry_span.into(),
        });
    }
    Ok(())
}

fn check_extern_fun_effect_contract(fun: &ast::FunDecl) -> Result<(), AnnotationError> {
    if let Some(eff_param) = &fun.eff_param {
        return Err(AnnotationError::ExternFunEffParamNotAllowed {
            span: eff_param.span.into(),
        });
    }

    if let Some(effects) = &fun.effects
        && !effects.terms.is_empty()
    {
        return Err(AnnotationError::ExternFunEffectsNotAllowed {
            span: effects.span.into(),
        });
    }

    Ok(())
}

fn check_extern_fun_modifier_contract(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
    extern_abi: ExternAbi,
) -> Result<(), AnnotationError> {
    for ann in annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        let annotation = match kind {
            BuiltinAnnotationKind::Unsafe | BuiltinAnnotationKind::NoGC => {
                format!("@{}", kind.name())
            }
            _ => continue,
        };
        let (_, name_span) = annotation_name_and_span(source, ann);
        return Err(match extern_abi {
            ExternAbi::C => AnnotationError::ExternFunCAbiModifierRedundant {
                annotation,
                span: name_span.into(),
            },
            ExternAbi::Scoop => AnnotationError::ExternFunScoopAbiModifierNotSupported {
                annotation,
                span: name_span.into(),
            },
        });
    }

    Ok(())
}

fn check_extern_fun_scoop_v1_decl_shape(
    fun: &ast::FunDecl,
    fun_site: ExternFunctionSite,
) -> Result<(), AnnotationError> {
    if fun_site != ExternFunctionSite::TopLevel || fun.receiver.is_some() {
        let span = fun
            .receiver
            .as_ref()
            .map(|receiver| receiver.span())
            .unwrap_or(fun.name.span);
        return Err(AnnotationError::ExternFunScoopAbiRequiresTopLevelFun { span: span.into() });
    }

    if let Some(type_param) = fun.type_params.first() {
        return Err(AnnotationError::ExternFunScoopAbiGenericsNotSupported {
            span: type_param.span.into(),
        });
    }

    Ok(())
}

fn check_extern_fun_signature_matches_scoop_abi_v1(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    for p in &fun.params {
        let Some(ty_ref) = p.ty.as_ref() else {
            continue;
        };
        check_extern_abi_type_ref_matches_scoop_abi_v1(source, ty_ref, lower)?;
    }

    if let Some(ret_ty_ref) = fun.return_ty.as_ref() {
        check_extern_abi_type_ref_matches_scoop_abi_v1(source, ret_ty_ref, lower)?;
    }

    Ok(())
}

fn check_extern_abi_type_ref_matches_scoop_abi_v1(
    _source: &SourceFile,
    ty_ref: &ast::TypeRef,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    let ty = match lower.lower_type_ref(ty_ref) {
        Ok(ty) => ty,
        Err(_e) => return Ok(()),
    };

    if scoop_abi_v1_type_is_supported(ty, lower) {
        return Ok(());
    }

    Err(
        AnnotationError::ExternFunScoopAbiCallableSurfaceNotSupported {
            found: lower.fmt_type(ty),
            span: ty_ref.span().into(),
        },
    )
}

fn scoop_abi_v1_type_is_supported(ty: TypeId, lower: &mut TypeLowering<'_>) -> bool {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Function(_)) => false,
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            !is_continuation_nominal(nominal.fqn.as_str())
        }
        TypeKind::Ref(_) | TypeKind::Param(_) => true,
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            scoop_abi_v1_type_is_supported(inner, lower)
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements
            .iter()
            .copied()
            .all(|element| scoop_abi_v1_type_is_supported(element, lower)),
        TypeKind::Value(_) => true,
        TypeKind::StarProjection(_) => false,
    }
}

fn is_continuation_nominal(fqn: &str) -> bool {
    fqn == "scoop.core.Continuation"
}

fn check_extern_fun_signature_matches_native_abi(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    // receiver：`fun Receiver.name(...)` 语义上等价于第一个 native ABI 参数。
    if let Some(receiver) = fun.receiver.as_ref() {
        check_extern_abi_type_ref_matches_native_abi(source, receiver, lower)?;
    }

    for p in &fun.params {
        let Some(ty_ref) = p.ty.as_ref() else {
            // 缺失类型由其它检查负责（保持健壮性）。
            continue;
        };
        check_extern_abi_type_ref_matches_native_abi(source, ty_ref, lower)?;
    }

    // 缺省 return 为 Unit：天然属于当前 native surface。
    if let Some(ret_ty_ref) = fun.return_ty.as_ref() {
        check_extern_abi_type_ref_matches_native_abi(source, ret_ty_ref, lower)?;
    }

    Ok(())
}

fn check_calling_convention_fun_contract(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
    _args: ParsedCallingConventionAnnotationArgs,
) -> Result<(), AnnotationError> {
    if matches!(fun.body, ast::FunBody::Missing) {
        let fun_name = source.slice(fun.name.span).to_string();
        return Err(AnnotationError::CallingConventionFunMustHaveBody {
            fun_name,
            span: fun.name.span.into(),
        });
    }

    if let Some(type_param) = fun.type_params.first() {
        return Err(AnnotationError::CallingConventionFunGenericsNotSupported {
            span: type_param.span.into(),
        });
    }

    check_calling_convention_fun_effect_contract(fun)?;
    check_calling_convention_fun_signature_matches_native_abi(source, fun, lower)
}

fn check_calling_convention_fun_effect_contract(fun: &ast::FunDecl) -> Result<(), AnnotationError> {
    if let Some(eff_param) = &fun.eff_param {
        return Err(AnnotationError::CallingConventionFunEffParamNotAllowed {
            span: eff_param.span.into(),
        });
    }

    if let Some(effects) = &fun.effects
        && !effects.terms.is_empty()
    {
        return Err(AnnotationError::CallingConventionFunEffectsNotAllowed {
            span: effects.span.into(),
        });
    }

    Ok(())
}

fn check_calling_convention_fun_signature_matches_native_abi(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    if let Some(receiver) = fun.receiver.as_ref() {
        check_calling_convention_type_ref_matches_native_abi(source, receiver, lower)?;
    }

    for p in &fun.params {
        let Some(ty_ref) = p.ty.as_ref() else {
            continue;
        };
        check_calling_convention_type_ref_matches_native_abi(source, ty_ref, lower)?;
    }

    if let Some(ret_ty_ref) = fun.return_ty.as_ref() {
        check_calling_convention_type_ref_matches_native_abi(source, ret_ty_ref, lower)?;
    }

    Ok(())
}

fn check_calling_convention_type_ref_matches_native_abi(
    _source: &SourceFile,
    ty_ref: &ast::TypeRef,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    let ty = match lower.lower_type_ref(ty_ref) {
        Ok(ty) => ty,
        Err(_e) => return Ok(()),
    };

    let is_native_abi = match lower.is_native_abi_value_type(ty) {
        Ok(v) => v,
        Err(_e) => return Ok(()),
    };

    if is_native_abi {
        return Ok(());
    }

    Err(
        AnnotationError::CallingConventionFunSignatureNotSupportedByNativeAbi {
            found: lower.fmt_type(ty),
            span: ty_ref.span().into(),
        },
    )
}

fn check_extern_abi_type_ref_matches_native_abi(
    _source: &SourceFile,
    ty_ref: &ast::TypeRef,
    lower: &mut TypeLowering<'_>,
) -> Result<(), AnnotationError> {
    let ty = match lower.lower_type_ref(ty_ref) {
        Ok(ty) => ty,
        Err(_e) => return Ok(()),
    };

    let is_native_abi = match lower.is_native_abi_value_type(ty) {
        Ok(v) => v,
        Err(_e) => return Ok(()),
    };

    if is_native_abi {
        return Ok(());
    }

    Err(AnnotationError::ExternFunSignatureNotSupportedByNativeAbi {
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
            BuiltinAnnotationKind::Deprecated
            | BuiltinAnnotationKind::Suppress
            | BuiltinAnnotationKind::Experimental => {}
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
        match kind {
            BuiltinAnnotationKind::Extern => {
                check_extern_builtin_annotation_args(
                    source,
                    ann,
                    ExternAnnotationTarget::NonFunction,
                )?;
            }
            BuiltinAnnotationKind::Deprecated
            | BuiltinAnnotationKind::Suppress
            | BuiltinAnnotationKind::Experimental => {}
            _ => {
                let (_, name_span) = annotation_name_and_span(source, ann);
                return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                    annotation: format!("@{}", kind.name()),
                    allowed: kind.allowed_targets_hint(),
                    found: "val/var",
                    span: name_span.into(),
                });
            }
        }
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
) -> Result<ParsedCallingConventionAnnotationArgs, AnnotationError> {
    if ann.args.is_empty() {
        return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
            span: ann.span.into(),
        });
    }

    let mut parsed = ParsedCallingConventionAnnotationArgs::default();
    let mut convention_arg: Option<Span> = None;
    let mut name_arg: Option<Span> = None;
    let mut seen_named = false;

    for arg in &ann.args {
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
                _ => {
                    if seen_named || convention_arg.is_some() {
                        return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
                            span: arg.span.into(),
                        });
                    }
                    let Some(name) = extract_string_literal_text(source, &arg.value) else {
                        return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
                            span: arg.value.span.into(),
                        });
                    };
                    check_calling_convention_name(&name, arg.value.span)?;
                    convention_arg = Some(arg.span);
                    parsed.convention_span = Some(arg.value.span);
                    continue;
                }
            },
        };

        seen_named = true;
        let Some(value_text) = extract_string_literal_text(source, value) else {
            return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
                span: value.span.into(),
            });
        };

        match key {
            Some("name") => {
                if name_arg.is_some() {
                    return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
                        span: key_span.into(),
                    });
                }
                name_arg = Some(key_span);
            }
            Some("convention") => {
                if convention_arg.is_some() {
                    return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
                        span: key_span.into(),
                    });
                }
                check_calling_convention_name(&value_text, value.span)?;
                convention_arg = Some(key_span);
                parsed.convention_span = Some(value.span);
            }
            _ => {
                return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
                    span: key_span.into(),
                });
            }
        }
    }

    if parsed.convention_span.is_none() {
        return Err(AnnotationError::CallingConventionAnnotationArgsInvalid {
            span: ann.span.into(),
        });
    }

    Ok(parsed)
}

fn check_calling_convention_name(name: &str, span: Span) -> Result<(), AnnotationError> {
    let normalized = name.trim().to_ascii_lowercase();
    if normalized != "c" && normalized != "cdecl" {
        return Err(AnnotationError::CallingConventionNotSupported {
            name: name.to_string(),
            span: span.into(),
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
    let ty = match lower.with_warning_emission_suspended(|lower| {
        lower.lower_type_fqn_with_args(type_fqn.to_string(), Vec::new(), decl.name.span)
    }) {
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
    target: ExternAnnotationTarget,
) -> Result<ParsedExternAnnotationArgs, AnnotationError> {
    if ann.args.is_empty() {
        return Ok(ParsedExternAnnotationArgs::default());
    }

    let mut positional: Option<Span> = None;
    let mut name_arg: Option<Span> = None;
    let mut lib_arg: Option<Span> = None;
    let mut abi_arg: Option<Span> = None;
    let mut calling_convention_arg: Option<Span> = None;
    let mut seen_named = false;
    let mut parsed = ParsedExternAnnotationArgs::default();

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
                    return Err(AnnotationError::ExternAnnotationArgDuplicate {
                        param: "name",
                        span: key_span.into(),
                    });
                }
                name_arg = Some(key_span);
            }
            "lib" => {
                if lib_arg.is_some() {
                    return Err(AnnotationError::ExternAnnotationArgDuplicate {
                        param: "lib",
                        span: key_span.into(),
                    });
                }
                lib_arg = Some(key_span);
            }
            "abi" => {
                if abi_arg.is_some() {
                    return Err(AnnotationError::ExternAnnotationArgDuplicate {
                        param: "abi",
                        span: key_span.into(),
                    });
                }
                if target != ExternAnnotationTarget::Function {
                    return Err(
                        AnnotationError::ExternAnnotationAbiOnlySupportedOnFunctions {
                            span: key_span.into(),
                        },
                    );
                }
                let Some(abi_name) = extract_string_literal_text(source, value) else {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: value.span.into(),
                    });
                };
                let Some(abi) = ExternAbi::parse(&abi_name) else {
                    return Err(AnnotationError::ExternAnnotationAbiNotSupported {
                        name: abi_name,
                        span: value.span.into(),
                    });
                };
                abi_arg = Some(key_span);
                parsed.abi = abi;
            }
            "callingConvention" => {
                if calling_convention_arg.is_some() {
                    return Err(AnnotationError::ExternAnnotationArgDuplicate {
                        param: "callingConvention",
                        span: key_span.into(),
                    });
                }
                if target != ExternAnnotationTarget::Function {
                    return Err(
                        AnnotationError::ExternAnnotationCallingConventionOnlySupportedOnFunctions {
                            span: key_span.into(),
                        },
                    );
                }
                let Some(convention_name) = extract_string_literal_text(source, value) else {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: value.span.into(),
                    });
                };
                check_calling_convention_name(&convention_name, value.span)?;
                calling_convention_arg = Some(key_span);
                parsed.calling_convention_span = Some(value.span);
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

    Ok(parsed)
}

fn check_builtin_annotations_on_type_decl(
    source: &SourceFile,
    file_allows_intrinsic: bool,
    decl: &ast::TypeDecl,
    type_fqn: &str,
) -> Result<(), AnnotationError> {
    let flags = BuiltinAnnotationFlags::from_annotations(source, &decl.annotations);

    // type 声明目前只允许 `@Intrinsic`；其它内建注解的 target 语义留到后续任务补齐。
    for ann in &decl.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        match kind {
            BuiltinAnnotationKind::Intrinsic => {
                check_intrinsic_builtin_annotation_gate(
                    source,
                    file_allows_intrinsic,
                    ann,
                    "类型",
                    type_fqn,
                )?;
                if !ann.args.is_empty() {
                    let (_, name_span) = annotation_name_and_span(source, ann);
                    return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
                        annotation: format!("@{}", kind.name()),
                        span: name_span.into(),
                    });
                }
            }
            BuiltinAnnotationKind::Deprecated
            | BuiltinAnnotationKind::Suppress
            | BuiltinAnnotationKind::Experimental => {}
            _ => {
                let (_, name_span) = annotation_name_and_span(source, ann);
                return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                    annotation: format!("@{}", kind.name()),
                    allowed: kind.allowed_targets_hint(),
                    found: "type",
                    span: name_span.into(),
                });
            }
        }
    }

    // `@Intrinsic struct/class`：layout 仍由编译器内置，因此不允许声明会引入存储布局的字段。
    // 普通成员函数 body 则按常规 nominal path 继续进入后续 lowering/codegen。
    if flags.is_intrinsic {
        if let Some(primary_ctor) = &decl.primary_ctor {
            for param in &primary_ctor.params {
                if param.kind.is_some() {
                    let field_name = source.slice(param.name.span).to_string();
                    return Err(AnnotationError::IntrinsicTypeFieldNotSupported {
                        type_fqn: type_fqn.to_string(),
                        field_name,
                        span: param.name.span.into(),
                    });
                }
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(property) = member else {
                    continue;
                };
                if property.is_direct_field() {
                    let field_name = source.slice(property.name.span).to_string();
                    return Err(AnnotationError::IntrinsicTypeFieldNotSupported {
                        type_fqn: type_fqn.to_string(),
                        field_name,
                        span: property.name.span.into(),
                    });
                }
            }
        }
    }

    Ok(())
}

fn check_intrinsic_builtin_annotation_gate(
    source: &SourceFile,
    _file_allows_intrinsic: bool,
    ann: &ast::AnnotationUse,
    decl_kind: &'static str,
    decl_name: &str,
) -> Result<(), AnnotationError> {
    if source.is_trusted_syslib() {
        return Ok(());
    }

    let (_, name_span) = annotation_name_and_span(source, ann);
    Err(AnnotationError::IntrinsicDeclRequiresTrustedSyslib {
        decl_kind,
        decl_name: decl_name.to_string(),
        span: name_span.into(),
    })
}

fn check_builtin_annotations_on_object_decl(
    source: &SourceFile,
    file_allows_intrinsic: bool,
    obj: &ast::ObjectDecl,
    obj_fqn: &str,
) -> Result<(), AnnotationError> {
    for ann in &obj.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        match kind {
            BuiltinAnnotationKind::Extern => {
                check_extern_builtin_annotation_args(
                    source,
                    ann,
                    ExternAnnotationTarget::NonFunction,
                )?;
            }
            BuiltinAnnotationKind::Intrinsic => {
                check_intrinsic_builtin_annotation_gate(
                    source,
                    file_allows_intrinsic,
                    ann,
                    object_kind_name(obj.kind),
                    obj_fqn,
                )?;
                if !ann.args.is_empty() {
                    let (_, name_span) = annotation_name_and_span(source, ann);
                    return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
                        annotation: format!("@{}", kind.name()),
                        span: name_span.into(),
                    });
                }
            }
            BuiltinAnnotationKind::Deprecated
            | BuiltinAnnotationKind::Suppress
            | BuiltinAnnotationKind::Experimental => {}
            _ => {
                let (_, name_span) = annotation_name_and_span(source, ann);
                return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                    annotation: format!("@{}", kind.name()),
                    allowed: kind.allowed_targets_hint(),
                    found: "object",
                    span: name_span.into(),
                });
            }
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
