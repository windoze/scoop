//! 表达式类型检查（早期最小子集）。
//!
//! 已覆盖能力：
//! - （T0405）字面量最小推导：
//!   - `1` → `Int`
//!   - `"..."` / `f"..."` → `String`
//!   - `true` / `false` → `Bool`（当前阶段以 ident 语法承载）
//!   - `()` → `Unit`
//! - （T0406）变量引用（ident）类型推导：
//!   - 局部 `val/var`（通过 resolver 写回的 `ResolvedValueRef::Local`）
//!   - 函数参数（同样视作 `Local` 绑定）
//!   - 顶层 `val/var`（`ResolvedValueRef::TopLevel`，当前仅支持当前文件内可查询的顶层变量）
//! - （T0407）函数调用（`callee(args...)`）：
//!   - 参数数量检查
//!   - 参数类型匹配
//!   - 当前仅支持“当前文件内”的顶层函数（无重载/无默认参数/无命名参数）
//!
//! 说明：该模块以“可回归、可扩展”为目标，逐步把更多 `ExprKind`/`StmtKind` 纳入 typecheck。

use miette::Diagnostic;
use thiserror::Error;

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::monomorph::MonomorphKey;
use crate::resolve::{ConeId, ConstructorOverload, ImportTable, Index, Visibility};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::assignable::{is_type_assignable, nominal_is_subtype_by_fqn};
use super::branch_merge;
use super::eff_row_subst::{
    EffRowVarSubstPlan, apply_eff_row_var_subst_plan, build_eff_row_var_subst_plan,
};
use super::lower::{TypeLowerError, TypeLowering};
use super::val_pat;
use super::when_exhaustiveness;
use super::when_pat;
use super::{TypeEnv, TypeSymbolKind, type_env::EnumVariantInfo};
use super::builtin_annotations::BuiltinAnnotationFlags;

const ASYNC_EFFECT_FQN: &str = "scoop.core.Async";
const TASK_FQN: &str = "scoop.core.Task";

#[derive(Debug, Error, Diagnostic)]
pub enum ExprTypeError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error("暂不支持的表达式类型检查：{kind}")]
    #[diagnostic(code(scoop::typecheck::unsupported_expr))]
    UnsupportedExpr {
        kind: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "无法推断 lambda 参数类型：参数 `{param}` 缺少类型注解，且当前语境没有期望的函数类型（约束来源：期望函数类型）"
    )]
    #[diagnostic(code(scoop::typecheck::lambda_param_type_not_inferred))]
    LambdaParamTypeNotInferred {
        param: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("缺少效果声明：函数声明为 {declared}，但这里 perform 了 {required}")]
    #[diagnostic(code(scoop::typecheck::required_effect_not_declared))]
    RequiredEffectNotDeclared {
        required: String,
        declared: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("程序入口 `main` 必须为 Pure（不能声明为 {declared}）")]
    #[diagnostic(code(scoop::typecheck::entry_point_must_be_pure))]
    EntryPointMustBePure {
        declared: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "程序入口 `main` 必须为闭合 effect row：`Pure!`（这里写的是 {declared}，请在 row 末尾加 `!`）"
    )]
    #[diagnostic(code(scoop::typecheck::entry_point_must_be_closed_pure))]
    EntryPointMustBeClosedPure {
        declared: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的模式绑定（pattern binding）")]
    #[diagnostic(code(scoop::typecheck::unsupported_pattern_binding))]
    UnsupportedPatternBinding {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("解构绑定仅允许 `val`，不允许 `var`")]
    #[diagnostic(code(scoop::typecheck::destructuring_var_not_allowed))]
    DestructuringVarNotAllowed {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 tuple pattern 只能用于 tuple/Unit，但 initializer 为 {found}")]
    #[diagnostic(code(scoop::typecheck::val_tuple_pat_not_tuple))]
    ValTuplePatNotTuple {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 tuple pattern 长度不匹配：期望 {expected} 个元素，但得到 {found} 个")]
    #[diagnostic(code(scoop::typecheck::val_tuple_pat_arity_mismatch))]
    ValTuplePatArityMismatch {
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`val` 解构的 tuple pattern 需要至少 {expected_at_least} 个元素，但 initializer 只有 {found} 个"
    )]
    #[diagnostic(code(scoop::typecheck::val_tuple_pat_too_short))]
    ValTuplePatTooShort {
        expected_at_least: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 struct pattern 类型必须是 struct，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::val_struct_pat_not_struct))]
    ValStructPatNotStruct {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 struct pattern 类型不匹配：期望 {expected}，但 initializer 为 {found}")]
    #[diagnostic(code(scoop::typecheck::val_struct_pat_type_mismatch))]
    ValStructPatTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 struct pattern 字段重复：{struct_name}.{field}")]
    #[diagnostic(code(scoop::typecheck::val_struct_pat_duplicate_field))]
    ValStructPatDuplicateField {
        struct_name: String,
        field: String,
        #[label("重复写在这里")]
        second: miette::SourceSpan,
        #[label("第一次写在这里")]
        first: miette::SourceSpan,
    },

    #[error("`{struct_name}` 不存在字段：{field}")]
    #[diagnostic(code(scoop::typecheck::val_struct_pat_unknown_field))]
    ValStructPatUnknownField {
        struct_name: String,
        field: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 struct pattern 缺少字段：{struct_name} 还需要 {fields}")]
    #[diagnostic(code(scoop::typecheck::val_struct_pat_missing_fields))]
    ValStructPatMissingFields {
        struct_name: String,
        fields: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 variant pattern 只能用于 enum，但 initializer 为 {found}")]
    #[diagnostic(code(scoop::typecheck::val_variant_pat_not_enum))]
    ValVariantPatNotEnum {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 variant pattern enum 前缀不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::val_variant_pat_enum_mismatch))]
    ValVariantPatEnumMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 解构的 variant pattern 未找到匹配的 variant：{enum_fqn}.{variant}")]
    #[diagnostic(code(scoop::typecheck::val_variant_pat_unknown_variant))]
    ValVariantPatUnknownVariant {
        enum_fqn: String,
        variant: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`val` 解构的 variant pattern 参数数量不匹配：{variant_fqn} 期望 {expected} 个，但得到 {found} 个"
    )]
    #[diagnostic(code(scoop::typecheck::val_variant_pat_arity_mismatch))]
    ValVariantPatArityMismatch {
        variant_fqn: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`val` 解构的 variant pattern 参数不足：{variant_fqn} 需要至少 {expected_at_least} 个，但该 variant 只有 {found} 个"
    )]
    #[diagnostic(code(scoop::typecheck::val_variant_pat_too_short))]
    ValVariantPatTooShort {
        variant_fqn: String,
        expected_at_least: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("无法获取局部绑定的类型：{name}")]
    #[diagnostic(code(scoop::typecheck::unknown_local_value_type))]
    UnknownLocalValueType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的顶层值引用类型推导：{fqn}")]
    #[diagnostic(code(scoop::typecheck::unsupported_top_level_value_type))]
    UnsupportedTopLevelValueType {
        fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("初始化表达式类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::initializer_type_mismatch))]
    InitializerTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "if 分支类型不匹配（{branch}）：期望 {expected}，但得到 {found}（约束来源：{expected_from}）"
    )]
    #[diagnostic(code(scoop::typecheck::if_branch_type_mismatch))]
    IfBranchTypeMismatch {
        branch: &'static str,
        expected: String,
        found: String,
        expected_from: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("不可调用：{callee}")]
    #[diagnostic(code(scoop::typecheck::callee_not_callable))]
    CalleeNotCallable {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("object 单例不可构造：{name}")]
    #[diagnostic(code(scoop::typecheck::object_not_constructible))]
    ObjectNotConstructible {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用解析歧义：{callee}")]
    #[diagnostic(code(scoop::typecheck::ambiguous_call))]
    AmbiguousCall {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("重载决议歧义：{callee}（候选：{candidates}）")]
    #[diagnostic(code(scoop::typecheck::ambiguous_overload))]
    AmbiguousOverload {
        callee: String,
        candidates: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("没有匹配的重载：{callee}")]
    #[diagnostic(code(scoop::typecheck::no_matching_overload))]
    NoMatchingOverload {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用 `@Extern` 函数需要 unsafe context：{callee}")]
    #[diagnostic(code(scoop::typecheck::extern_call_requires_unsafe))]
    ExternCallRequiresUnsafeContext {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用 `@Unsafe` 函数需要 unsafe context：{callee}")]
    #[diagnostic(code(scoop::typecheck::unsafe_call_requires_unsafe))]
    UnsafeCallRequiresUnsafeContext {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用参数数量不匹配：{callee} 期望 {expected} 个，但提供了 {found} 个")]
    #[diagnostic(code(scoop::typecheck::call_arity_mismatch))]
    CallArityMismatch {
        callee: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用参数类型不匹配：{callee} 第 {index} 个参数期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::call_arg_type_mismatch))]
    CallArgTypeMismatch {
        callee: String,
        index: usize,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("无法推断泛型类型实参：{callee} 的 `{param}`（缺少可用于推断的调用点约束）")]
    #[diagnostic(code(scoop::typecheck::generic_type_arg_not_inferred))]
    GenericTypeArgNotInferred {
        callee: String,
        param: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "泛型类型实参推断冲突：{callee} 的 `{param}` 同时被约束为 {left}（来自 {left_from}）与 {right}（来自 {right_from}）（约束来源：调用点实参）"
    )]
    #[diagnostic(code(scoop::typecheck::generic_type_arg_inference_conflict))]
    GenericTypeArgInferenceConflict {
        callee: String,
        param: String,
        left: String,
        right: String,
        left_from: String,
        right_from: String,
        #[label("这里（产生冲突的约束）")]
        span: miette::SourceSpan,
        #[label("这里（之前的约束）")]
        previous: miette::SourceSpan,
    },

    #[error("enum variant 构造歧义：{name} 同时匹配 {candidates}")]
    #[diagnostic(code(scoop::typecheck::ambiguous_enum_variant_ctor))]
    AmbiguousEnumVariantCtor {
        name: String,
        candidates: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("enum variant 构造参数数量不匹配：{variant} 期望 {expected} 个，但提供了 {found} 个")]
    #[diagnostic(code(scoop::typecheck::enum_variant_ctor_arity_mismatch))]
    EnumVariantCtorArityMismatch {
        variant: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "enum variant 构造参数类型不匹配：{variant} 第 {index} 个参数期望 {expected}，但得到 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::enum_variant_ctor_arg_type_mismatch))]
    EnumVariantCtorArgTypeMismatch {
        variant: String,
        index: usize,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("次构造器缺少 delegation call：{class_fqn} 的 `constructor(...)` 必须写 `: this(...)`")]
    #[diagnostic(code(scoop::typecheck::secondary_ctor_delegation_required))]
    SecondaryCtorDelegationRequired {
        class_fqn: String,
        #[label("这里需要写 `: this(...)`")]
        span: miette::SourceSpan,
    },

    #[error("次构造器 delegation 非法：{class_fqn} 有主构造器时只能委托到 `this(...)`")]
    #[diagnostic(code(scoop::typecheck::secondary_ctor_delegation_must_be_this))]
    SecondaryCtorDelegationMustBeThis {
        class_fqn: String,
        #[label("这里必须是 `this(...)`")]
        span: miette::SourceSpan,
    },

    #[error("无法推断 enum 类型参数：{enum_fqn} 的 `{param}`")]
    #[diagnostic(code(scoop::typecheck::enum_variant_ctor_type_arg_not_inferred))]
    EnumVariantCtorTypeArgNotInferred {
        enum_fqn: String,
        param: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 的 tuple pattern 只能用于 tuple/Unit，但 subject 为 {found}")]
    #[diagnostic(code(scoop::typecheck::when_tuple_pat_not_tuple))]
    WhenTuplePatNotTuple {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 的 tuple pattern 长度不匹配：期望 {expected} 个元素，但得到 {found} 个")]
    #[diagnostic(code(scoop::typecheck::when_tuple_pat_arity_mismatch))]
    WhenTuplePatArityMismatch {
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`when` 的 tuple pattern 需要至少 {expected_at_least} 个元素，但 subject 只有 {found} 个"
    )]
    #[diagnostic(code(scoop::typecheck::when_tuple_pat_too_short))]
    WhenTuplePatTooShort {
        expected_at_least: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 的 variant pattern 只能用于 enum，但 subject 为 {found}")]
    #[diagnostic(code(scoop::typecheck::when_variant_pat_not_enum))]
    WhenVariantPatNotEnum {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 的 variant pattern 未找到匹配的 variant：{enum_fqn}.{variant}")]
    #[diagnostic(code(scoop::typecheck::when_variant_pat_unknown_variant))]
    WhenVariantPatUnknownVariant {
        enum_fqn: String,
        variant: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`when` 的 variant pattern 参数数量不匹配：{variant_fqn} 期望 {expected} 个，但得到 {found} 个"
    )]
    #[diagnostic(code(scoop::typecheck::when_variant_pat_arity_mismatch))]
    WhenVariantPatArityMismatch {
        variant_fqn: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`when` 的 variant pattern 参数不足：{variant_fqn} 需要至少 {expected_at_least} 个，但该 variant 只有 {found} 个"
    )]
    #[diagnostic(code(scoop::typecheck::when_variant_pat_too_short))]
    WhenVariantPatTooShort {
        variant_fqn: String,
        expected_at_least: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 分支不穷尽：缺少 {subject} 的 {missing}")]
    #[diagnostic(code(scoop::typecheck::when_non_exhaustive_missing_variants))]
    WhenNonExhaustiveMissingVariants {
        subject: String,
        missing: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 在 {subject} 上不是穷尽的：必须包含 `else` 或 `_`")]
    #[diagnostic(code(scoop::typecheck::when_missing_else))]
    WhenMissingElse {
        subject: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用 receiver 类型不匹配：{callee} 期望 receiver 为 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::call_receiver_type_mismatch))]
    CallReceiverTypeMismatch {
        callee: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`?.` 的 receiver 必须是 nullable（`T?` / `Option<T>`），但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::safe_access_receiver_not_nullable))]
    SafeAccessReceiverNotNullable {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("Elvis `?:` 左操作数必须是 nullable（`T?` / `Option<T>`），但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::elvis_lhs_not_nullable))]
    ElvisLhsNotNullable {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("Elvis `?:` 右操作数类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::elvis_rhs_type_mismatch))]
    ElvisRhsTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`!!` 的操作数必须是 nullable（`T?` / `Option<T>`），但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::not_null_assert_operand_not_nullable))]
    NotNullAssertOperandNotNullable {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的成员访问：{fqn}")]
    #[diagnostic(code(scoop::typecheck::unsupported_member_access))]
    UnsupportedMemberAccess {
        fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("不允许的显式类型转换：{from} -> {to}")]
    #[diagnostic(code(scoop::typecheck::invalid_cast))]
    InvalidCast {
        from: String,
        to: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("一元运算符 `{op}` 的操作数类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::unary_op_operand_type_mismatch))]
    UnaryOpOperandTypeMismatch {
        op: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("二元运算符 `{op}` 的操作数类型不匹配：期望 {expected}，但 lhs 为 {lhs}、rhs 为 {rhs}")]
    #[diagnostic(code(scoop::typecheck::binary_op_operand_type_mismatch))]
    BinaryOpOperandTypeMismatch {
        op: String,
        expected: String,
        lhs: String,
        rhs: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("不可赋值：`{name}` 不是可变变量（必须声明为 `var`）")]
    #[diagnostic(code(scoop::typecheck::assignment_target_not_mutable))]
    AssignmentTargetNotMutable {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("赋值类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::assignment_type_mismatch))]
    AssignmentTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`with` 的 base 必须是值类型（struct/tuple/enum），当前实现仅支持 struct；但得到 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::with_update_base_not_supported))]
    WithUpdateBaseNotSupported {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持嵌套字段路径更新：{path}（当前仅支持单段字段名）")]
    #[diagnostic(code(scoop::typecheck::with_update_nested_path_not_supported))]
    WithUpdateNestedPathNotSupported {
        path: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`with` 更新字段路径重复：{path}")]
    #[diagnostic(code(scoop::typecheck::with_update_duplicate_path))]
    WithUpdateDuplicatePath {
        path: String,
        #[label("重复写在这里")]
        second: miette::SourceSpan,
        #[label("第一次写在这里")]
        first: miette::SourceSpan,
    },

    #[error("`with` 更新字段路径冲突：{parent} 与 {child}（并行语义不允许一条路径包含另一条）")]
    #[diagnostic(code(scoop::typecheck::with_update_overlapping_paths))]
    WithUpdateOverlappingPaths {
        parent: String,
        child: String,
        #[label("冲突写在这里")]
        second: miette::SourceSpan,
        #[label("已在这里更新过")]
        first: miette::SourceSpan,
    },

    #[error(
        "`with` 嵌套字段路径不可继续：`{struct_name}.{field}` 的类型必须是 struct，但得到 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::with_update_nested_path_not_struct))]
    WithUpdateNestedPathNotStruct {
        struct_name: String,
        field: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`{struct_name}` 不存在字段：{field}")]
    #[diagnostic(code(scoop::typecheck::with_update_unknown_field))]
    WithUpdateUnknownField {
        struct_name: String,
        field: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`{struct_name}.{field}` 更新值类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::with_update_field_type_mismatch))]
    WithUpdateFieldTypeMismatch {
        struct_name: String,
        field: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("struct literal 的类型必须是 struct，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_not_struct))]
    StructLitNotStruct {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`{struct_name}` 不存在字段：{field}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_unknown_field))]
    StructLitUnknownField {
        struct_name: String,
        field: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("struct literal 字段重复：{struct_name}.{field}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_duplicate_field))]
    StructLitDuplicateField {
        struct_name: String,
        field: String,
        #[label("重复写在这里")]
        second: miette::SourceSpan,
        #[label("第一次写在这里")]
        first: miette::SourceSpan,
    },

    #[error("`{struct_name}.{field}` 初始化值类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_field_type_mismatch))]
    StructLitFieldTypeMismatch {
        struct_name: String,
        field: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("struct literal 缺少字段：{struct_name} 还需要 {fields}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_missing_fields))]
    StructLitMissingFields {
        struct_name: String,
        fields: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("返回类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::return_type_mismatch))]
    ReturnTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("handler arm 的返回类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::handle_arm_return_type_mismatch))]
    HandleArmReturnTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("不可达的 handler arm：{current} 已被前面的 {previous} 覆盖")]
    #[diagnostic(code(scoop::typecheck::handle_arm_unreachable))]
    HandleArmUnreachable {
        previous: String,
        current: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("缺少返回值：函数返回类型为 {expected}")]
    #[diagnostic(code(scoop::typecheck::return_value_required))]
    ReturnValueRequired {
        expected: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`return` 只能出现在普通函数体内（lambda 的 non-local return 仅允许出现在 inline 函数的 lambda 实参中）"
    )]
    #[diagnostic(code(scoop::typecheck::return_not_in_function_body))]
    ReturnNotInFunctionBody {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`while` 条件类型必须是 Bool，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::while_condition_not_bool))]
    WhileConditionNotBool {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 分支 guard 条件类型必须是 Bool，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::when_guard_not_bool))]
    WhenGuardNotBool {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`break` 只能出现在循环体内")]
    #[diagnostic(code(scoop::typecheck::break_not_in_loop))]
    BreakNotInLoop {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`continue` 只能出现在循环体内")]
    #[diagnostic(code(scoop::typecheck::continue_not_in_loop))]
    ContinueNotInLoop {
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

#[derive(Debug, Clone)]
struct EffParamSig {
    name: String,
    default: EffectRow,
}

#[derive(Debug, Clone)]
struct FunSigOwned {
    /// 声明处 name 的 span：用于把“某个具体 overload”与 AST 节点对应起来，
    /// 以便在后续 pass 中回写（例如返回类型推断，T0507）。
    decl_span: Span,
    /// 是否为扩展函数（`fun Receiver.name(...)`）。
    ///
    /// 说明：
    /// - 在 typecheck 阶段我们把扩展函数“降糖”为普通顶层函数：receiver 作为第一个参数（spec §7.4）；
    /// - 该标记仅用于限制语法层的可调用性：扩展函数不能以 `f(args...)` 形式直接调用，
    ///   只能通过 `receiver.f(args...)` / `receiver?.f(args...)` 调用（当前阶段最小子集）。
    is_extension: bool,
    /// 是否为 `inline` 函数（spec §7.2/§7.3；TODO T0444）。
    ///
    /// 说明：当前阶段不做任何 inlining 优化，该标记仅用于：
    /// - lambda non-local return 的静态门禁（只有 inline lambda 实参允许 `return`）
    is_inline: bool,
    /// 是否为 `@Unsafe` 函数（spec §15.9.1）。
    ///
    /// 说明：当前阶段（T1003）仅用于调用门禁：非 unsafe context 禁止调用 `@Unsafe`。
    is_unsafe: bool,
    /// 是否为 `@NoGC` 函数（spec §15.8）。
    ///
    /// 说明：当前阶段不实现 “可能分配” 分析；但 `@Extern` 会隐含 `@NoGC`（在收集阶段折叠）。
    #[allow(dead_code)]
    is_nogc: bool,
    /// 是否为 `@Extern` 函数（spec §15.8.3）。
    ///
    /// 说明：当前阶段（T1003）仅用于调用门禁：非 unsafe context 禁止调用 `@Extern`。
    is_extern: bool,
    /// 是否为 `@Intrinsic` 函数（spec §15.7）。
    ///
    /// 说明：当前阶段仅记录该标记，供后续 lowering/codegen 使用。
    #[allow(dead_code)]
    is_intrinsic: bool,
    /// 形参名列表（与 `params` 对齐）。
    ///
    /// 用途：
    /// - T0453：命名实参（`name = expr`）的重排与匹配；
    /// - 未来：默认参数/重载决议可复用该信息。
    ///
    /// 说明：
    /// - 对于扩展函数，`params[0]` 是 receiver 的类型占位；该位置的 `param_names[0]`
    ///   仅用于对齐，当前不会参与命名实参匹配（因为 receiver 不可被命名传入）。
    param_names: Vec<String>,
    /// 形参是否带默认值（与 `params` 对齐）。
    ///
    /// 用途：
    /// - T0454：构造调用重载决议已经支持默认参数；
    /// - T0512：把默认参数纳入函数调用的 overload resolution（先只做“候选可用性/映射”，
    ///   默认值表达式的补齐语义留给后续任务 T1305）。
    ///
    /// 说明：
    /// - 对于扩展函数，`params[0]` 是 receiver，占位为 `false`；
    /// - 当前阶段这里只需要“是否存在默认值”，不复制默认值表达式本体。
    param_has_defaults: Vec<bool>,
    /// 函数级 type params（按声明顺序）。
    ///
    /// 用途（T0505）：
    /// - 让调用点可以识别“哪些 TypeId 是该函数的类型参数”
    /// - 在参数检查前做最小泛型实参推断，并对签名做 substitution（实例化）
    type_params: Vec<TypeId>,
    /// effect row 参数（`<eff E = Pure>`）（spec §3.4 / §14.7.3）。
    ///
    /// 说明：
    /// - 当前阶段仅支持单一 `eff` 参数（parser 已强制最多一个）；
    /// - 若调用点无法从 lambda 实参推断该 row，则回退到 `default`。
    eff_param: Option<EffParamSig>,
    /// 形参类型若为函数类型，且其 effects row 引用函数级 `eff` 变量，则记录其“base row”：
    /// 把 `E` 从 row 表达式中移除后剩余的常量项（已按声明处上下文 lowering）。
    ///
    /// 例：
    /// - `(...)->T / E`            => `Some(Pure)`（base 为空）
    /// - `(...)->T / (E + IO)`     => `Some(IO)`
    /// - `(...)->T / (IO + State)` => `None`（不引用 `E`）
    ///
    /// 对齐约定：该数组与 `params` 对齐（扩展函数包含 receiver 的占位参数）。
    param_fn_effect_eff_base: Vec<Option<EffectRow>>,
    /// 形参类型若为 `Type<eff Row>` 这类“use-site effect row 实参引用 `eff` 变量”的名义类型，
    /// 同样记录其 base row（把 `E` 移除后剩余的常量项）。
    ///
    /// 用途（T0624）：
    /// - 推断 `E` 时，除了从 lambda body 的 required effects 外，也需要从类型实参里提取约束：
    ///   `Disposable<eff Async>` 作为实参会让 `E` 至少包含 `Async`。
    /// - 在推断出 `E` 之后，还需要把签名里以默认值 lowering 的 `Type<eff E>` 回填为
    ///   `Type<eff E_arg>`，否则 call arg 的 assignable 检查会错误地用默认值对比。
    ///
    /// 对齐约定：该数组与 `params` 对齐（扩展函数包含 receiver 的占位参数）。
    param_nominal_eff_eff_base: Vec<Option<EffectRow>>,
    /// `E + ...` 的嵌套替换 plan：用于把签名类型中（包括 tuple/Option/多层 function type 等）的
    /// `E + base` 统一实例化为调用点的 `E_arg + base`（T0628b）。
    ///
    /// 对齐约定：与 `params` 对齐（扩展函数包含 receiver 的占位参数）。
    param_eff_row_var_subst: Vec<EffRowVarSubstPlan>,
    /// 返回类型中的 `E + ...` 嵌套替换 plan（T0628b）。
    return_eff_row_var_subst: EffRowVarSubstPlan,
    params: Vec<TypeId>,
    return_ty: TypeId,
    /// 函数声明处的 effect row 标注：`/ Pure` / `/ E` / `/ (E1 + E2)`（spec §5.8）。
    effects: Option<ast::EffectRowExpr>,
}

/// 对一个文件的表达式做最小类型检查。
///
/// 说明：
/// - 当前只覆盖能明确推导的字面量；
/// - 会进入函数体与 class 成员方法体，但对“普通表达式语句”的覆盖仍是增量推进：
///   只在需要时递归进入 block/if/when 等结构，以避免在语法/类型系统尚未齐全时引入大面积回归。
pub fn check_file_exprs(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    let _ = check_file_exprs_impl(
        source,
        file,
        index,
        imports,
        env,
        types,
        builtins,
        false,
    )?;
    Ok(())
}

/// 对一个文件的表达式做最小类型检查，并在成功时返回单态化（monomorphization）请求集合（T0712）。
///
/// 说明：
/// - 该入口会执行与 `check_file_exprs` 相同的类型检查；
/// - 额外收集“泛型函数调用”的实例化信息，供后续 monomorph pass 生成专用实例并做去重缓存。
pub fn check_file_exprs_with_monomorph_keys(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<Vec<MonomorphKey>, ExprTypeError> {
    check_file_exprs_impl(
        source,
        file,
        index,
        imports,
        env,
        types,
        builtins,
        true,
    )
}

fn check_file_exprs_impl(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    collect_monomorph: bool,
) -> Result<Vec<MonomorphKey>, ExprTypeError> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
    if collect_monomorph {
        lower.enable_monomorph_collection();
    }

    // 这里单独拷贝一份 package 前缀，避免在借用 `lower` 的同时再借用其字段导致借用冲突。
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    // T0629a：program boundary 的 entry point 需要对 cone 边界敏感。
    // 在多 cone 编译单元里，仅把 consumer cone 的 `main` 视为 entry point，
    // 避免把依赖 cone（库）里的同名 `main` 误判为 entry point。
    let file_cone = index.cone_of_source(source);
    let consumer_cone = index.consumer_cone();

    // 顶层 `val/var` 的类型表：用于在表达式里引用顶层变量时查询其声明类型。
    //
    // 当前阶段约束：
    // - 只支持“当前文件内”的顶层变量（因为 typecheck phase 目前只解析单文件 AST）；
    // - 顶层变量必须有显式类型注解（由 `typecheck::check_file_headers` 保证）。
    let top_level_types = collect_top_level_value_types(source, file, &mut lower)?;
    let mut top_level_funs = collect_top_level_fun_signatures(source, file, &mut lower, builtins)?;
    let struct_field_types = collect_struct_field_types(source, file, &mut lower)?;
    let member_mutabilities = collect_member_mutabilities(source, file);

    for item in &file.items {
        match item {
            ast::Item::Val(v) => check_top_level_val_initializer(
                source,
                v,
                &mut lower,
                builtins,
                &top_level_types,
                &top_level_funs,
                &struct_field_types,
            )?,
            ast::Item::Fun(fun) => {
                let local_name = source.slice(fun.name.span);
                let fun_fqn = if pkg_prefix.is_empty() {
                    local_name.to_string()
                } else {
                    format!("{pkg_prefix}.{local_name}")
                };
                let is_entry_point = file_cone == consumer_cone
                    && fun.kind == ast::FunDeclKind::Regular
                    && fun.receiver.is_none()
                    && local_name == "main";

                check_fun_body_exprs(
                    source,
                    &fun_fqn,
                    fun,
                    is_entry_point,
                    &mut lower,
                    builtins,
                    &top_level_types,
                    &mut top_level_funs,
                    &member_mutabilities,
                    &struct_field_types,
                )?;
            }
            ast::Item::Type(ty) => check_class_member_fun_bodies_in_type_decl(
                source,
                ty,
                &pkg_prefix,
                &mut lower,
                builtins,
                &top_level_types,
                &top_level_funs,
                &member_mutabilities,
                &struct_field_types,
            )?,
            ast::Item::ExtensionProperty(_) | ast::Item::Object(_) | ast::Item::TypeAlias(_) => {}
        }
    }

    Ok(lower.take_monomorph_keys())
}

fn try_infer_fun_return_ty_from_block(
    source: &SourceFile,
    body: &ast::Block,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    loop_depth: usize,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Option<TypeId>, ExprTypeError> {
    // T0507：返回类型推断（最小实现）。
    //
    // 当前阶段只支持：
    // - “无显式 return”且 block 以表达式语句结尾：以最后表达式类型作为返回类型；
    // - “唯一的 return”且它是函数体最后一条语句：以该 return 的值类型作为返回类型；
    //
    // 其它情况（多 return、return 不在末尾、或最后表达式暂不可推导）先不推断，保持兼容旧行为：
    // 返回类型仍视为 `Unit`，并由现有的 `return_type_mismatch` 等错误兜底。

    // 注意：返回类型推断依赖“最后表达式 / return value”的类型推导，
    // 而这些表达式往往会引用在函数体中声明的局部变量：
    //
    // ```
    // fun f() {
    //   val x: Any = ...
    //   if (x is String) { ... }
    // }
    // ```
    //
    // 因此这里必须按语句顺序“先走一遍最小语句 typecheck”，把局部绑定写进 `locals`，
    // 再去推导最后表达式/return 的类型；否则会出现 `unknown_local_value_type` 的假错误。

    // 与 resolver 的作用域规则对齐：block 内声明仅在该 block 内可见。
    // 这里与 `check_block_exprs` 一样用“进入时快照 + 退出时回滚”实现。
    let saved_locals = locals.clone();
    let saved_stable = stable_bindings.clone();
    let saved_mutable = mutable_bindings.clone();

    let mut top_level_return_count = 0usize;
    let mut last_return_ty: Option<TypeId> = None;
    let mut tail_expr_ty: Option<TypeId> = None;

    for (idx, stmt) in body.stmts.iter().enumerate() {
        let is_last = idx + 1 == body.stmts.len();

        match &stmt.kind {
            ast::StmtKind::Return { value, .. } => {
                top_level_return_count += 1;
                if is_last {
                    last_return_ty = Some(match value {
                        Some(v) => infer_expr_type(
                            source,
                            v,
                            lower,
                            builtins,
                            locals,
                            top_level_types,
                            top_level_funs,
                            struct_field_types,
                        )?,
                        None => builtins.unit,
                    });
                }
                // 说明：这里刻意不做 `return` 的“类型匹配检查”，因为 expected return type 尚未确定。
                // 真正的 `return` 校验由下方第二遍 `check_block_exprs` 完成。
            }
            ast::StmtKind::Expr(e) => {
                // 先执行现有的“语句层递归”检查（smart cast / lambda return 门禁等）。
                check_expr_stmt(
                    source,
                    e,
                    lower,
                    builtins,
                    locals,
                    stable_bindings,
                    mutable_bindings,
                    loop_depth,
                    Some(builtins.unit),
                    top_level_types,
                    top_level_funs,
                    member_mutabilities,
                    struct_field_types,
                )?;

                if is_last {
                    match infer_expr_type(
                        source,
                        e,
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    ) {
                        Ok(ty) => tail_expr_ty = Some(ty),
                        Err(ExprTypeError::UnsupportedExpr { .. }) => {
                            // 兼容：statement position 的表达式当前并不总是完整 typecheck；
                            // 若仅因为“未实现某个 ExprKind”而失败，则不启用返回类型推断。
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            _ => {
                // 其它语句：复用现有逻辑以便正确更新 locals/stable/mutable，并递归覆盖子结构。
                check_stmt_exprs(
                    source,
                    stmt,
                    lower,
                    builtins,
                    locals,
                    stable_bindings,
                    mutable_bindings,
                    loop_depth,
                    Some(builtins.unit),
                    top_level_types,
                    top_level_funs,
                    member_mutabilities,
                    struct_field_types,
                )?;
            }
        }
    }

    *locals = saved_locals;
    *stable_bindings = saved_stable;
    *mutable_bindings = saved_mutable;

    // 推断规则（最小子集）：
    // - 唯一的 top-level return 且它是最后一条语句：返回该 return 的值类型
    // - 没有 top-level return：返回最后表达式语句的类型
    // - 其它情况暂不推断
    if top_level_return_count == 1 {
        Ok(last_return_ty)
    } else if top_level_return_count == 0 {
        Ok(tail_expr_ty)
    } else {
        Ok(None)
    }
}

fn check_class_member_fun_bodies_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    // 仅在 class 内启用 member fun body typecheck（T0438）。
    if matches!(decl.kind, ast::TypeKind::Class) {
        let ctor_params: &[ast::Param] = decl
            .primary_ctor
            .as_ref()
            .map(|c| c.params.as_slice())
            .unwrap_or(&[]);

        // `this` 在 class 成员体中可见：resolver 会把 `this` 解析到 `decl.name.span`（T0313）。
        //
        // class 的 type params 在成员体内可见：
        // - 让 `this` 的类型可表示为 `C<T, ...>`（而不是 `C<Any, ...>` 占位）；
        // - 让成员体内出现的 `as T` / `is T` 等 type position 能通过 lowering。
        //
        // 这同样避免了 `where` 约束满足性检查（T0458）对 “未知实参” 的误报：
        // `this: C<T>` 中的 `T` 是 `TypeKind::Param`，约束在该层被视作假设而非此刻验证的条件。
        lower.push_type_params(&decl.type_params);

        let result: Result<(), ExprTypeError> = (|| {
            let this_ty_args = decl
                .type_params
                .iter()
                .map(|p| lower.ty_param_from_decl(p))
                .collect::<Vec<_>>();
            let this_ty =
                lower.lower_type_fqn_with_args(type_fqn.clone(), this_ty_args, decl.name.span)?;

            if let Some(body) = &decl.body {
                for member in &body.members {
                    match member {
                        ast::TypeMember::Fun(fun) => {
                            check_class_member_fun_body_exprs(
                                source,
                                decl.name.span,
                                this_ty,
                                ctor_params,
                                fun,
                                lower,
                                builtins,
                                top_level_types,
                                top_level_funs,
                                member_mutabilities,
                                struct_field_types,
                            )?;
                        }
                        ast::TypeMember::Property(p) => {
                            check_class_property_initializer_exprs(
                                source,
                                decl.name.span,
                                this_ty,
                                ctor_params,
                                p,
                                lower,
                                builtins,
                                top_level_types,
                                top_level_funs,
                                struct_field_types,
                            )?;
                        }
                        ast::TypeMember::InitBlock(b) => {
                            check_class_init_block_exprs(
                                source,
                                decl.name.span,
                                this_ty,
                                ctor_params,
                                b,
                                lower,
                                builtins,
                                top_level_types,
                                top_level_funs,
                                member_mutabilities,
                                struct_field_types,
                            )?;
                        }
                        ast::TypeMember::SecondaryCtor(ctor) => {
                            check_class_secondary_ctor_exprs(
                                source,
                                decl.name.span,
                                &type_fqn,
                                decl.primary_ctor.is_some(),
                                this_ty,
                                ctor_params,
                                ctor,
                                lower,
                                builtins,
                                top_level_types,
                                top_level_funs,
                                member_mutabilities,
                                struct_field_types,
                            )?;
                        }
                        ast::TypeMember::EnumVariant(_)
                        | ast::TypeMember::Type(_)
                        | ast::TypeMember::Object(_) => {}
                    }
                }
            }

            Ok(())
        })();

        lower.pop_type_params(&decl.type_params);
        result?;
    }

    // 递归处理 nested types（可能存在 nested class）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            let ast::TypeMember::Type(nested) = member else {
                continue;
            };
            check_class_member_fun_bodies_in_type_decl(
                source,
                nested,
                &type_fqn,
                lower,
                builtins,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
        }
    }

    Ok(())
}

fn check_class_member_fun_body_exprs(
    source: &SourceFile,
    this_decl_span: Span,
    this_ty: TypeId,
    ctor_params: &[ast::Param],
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    lower.push_type_params(&fun.type_params);
    let eff_binding_pushed = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => match lower.lower_effect_row_expr(Some(expr)) {
                Ok(row) => row,
                Err(e) => {
                    lower.pop_type_params(&fun.type_params);
                    return Err(e.into());
                }
            },
            None => EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    let unsafe_ctx_pushed = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations).is_unsafe;
    if unsafe_ctx_pushed {
        lower.push_unsafe_context();
    }

    lower.begin_effect_collection();
    let body_result: Result<(), ExprTypeError> = (|| {
        let mut locals: HashMap<Span, TypeId> = HashMap::new();
        let mut stable_bindings: HashSet<Span> = HashSet::new();
        let mut mutable_bindings: HashSet<Span> = HashSet::new();

        // `this`：resolver 使用 `decl.name.span` 作为 decl_span。
        locals.insert(this_decl_span, this_ty);
        stable_bindings.insert(this_decl_span);

        // 若该 member fun 本身是扩展函数（member extension），resolver 会把 `this` 解析到 receiver 的 span；
        // 这里沿用顶层扩展函数的处理方式：receiver 作为一个隐式稳定绑定。
        if let Some(receiver) = &fun.receiver {
            let receiver_ty = lower.lower_type_ref(receiver)?;
            locals.insert(receiver.span(), receiver_ty);
            stable_bindings.insert(receiver.span());
        }

        // 主构造参数：resolver 在 member fun 内把 ctor params 当作外层局部绑定（T0313）。
        for p in ctor_params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            let ty = lower.lower_type_ref(ty_ref)?;
            locals.insert(p.name.span, ty);
            stable_bindings.insert(p.name.span);
        }

        // member fun 自身的参数（与顶层 fun 保持一致）。
        for p in &fun.params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            let ty = lower.lower_type_ref(ty_ref)?;
            locals.insert(p.name.span, ty);
            stable_bindings.insert(p.name.span);
        }

        // 函数的期望返回类型：用于 `return expr?` 的检查。
        let expected_return_ty = match &fun.return_ty {
            Some(ret) => lower.lower_type_ref(ret)?,
            None => match &fun.body {
                ast::FunBody::Block(b) => try_infer_fun_return_ty_from_block(
                    source,
                    b,
                    lower,
                    builtins,
                    &mut locals,
                    &mut stable_bindings,
                    &mut mutable_bindings,
                    0,
                    top_level_types,
                    top_level_funs,
                    member_mutabilities,
                    struct_field_types,
                )?
                .unwrap_or(builtins.unit),
                ast::FunBody::Missing => builtins.unit,
            },
        };

        match &fun.body {
            ast::FunBody::Block(b) => check_block_exprs(
                source,
                b,
                lower,
                builtins,
                &mut locals,
                &mut stable_bindings,
                &mut mutable_bindings,
                0,
                Some(expected_return_ty),
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?,
            ast::FunBody::Missing => {}
        }

        Ok(())
    })();
    let performed_effects = lower.finish_effect_collection();

    let result = match body_result {
        Ok(()) => {
            // T0623：member `async fun` 同样需要把 `Async` 留在 Task 的计算语境内。
            let performed_for_decl = if fun.modifiers.contains(&ast::Modifier::Async) {
                let async_effect = lower.lower_type_fqn_with_args(
                    ASYNC_EFFECT_FQN.to_string(),
                    Vec::new(),
                    fun.name.span,
                )?;
                performed_effects
                    .iter()
                    .copied()
                    .filter(|(effect, _)| *effect != async_effect)
                    .collect::<Vec<_>>()
            } else {
                performed_effects.clone()
            };

            check_required_effects_for_fun_decl(fun, &performed_for_decl, false, lower)?;
            Ok(())
        }
        Err(e) => Err(e),
    };
    if eff_binding_pushed {
        lower.pop_effect_row_param_binding();
    }
    if unsafe_ctx_pushed {
        lower.pop_unsafe_context();
    }
    lower.pop_type_params(&fun.type_params);
    result
}

/// 检查 class 属性 initializer 的最小表达式类型（T0448）。
///
/// 说明：
/// - 仅覆盖 `= expr` initializer（delegate `by expr` 的表达式类型检查留给 delegated property lowering 任务）。
/// - initializer 处于 class 初始化语境：可见 `this` 与主构造参数（resolver 已写回 Local decl_span）。
fn check_class_property_initializer_exprs(
    source: &SourceFile,
    this_decl_span: Span,
    this_ty: TypeId,
    ctor_params: &[ast::Param],
    p: &ast::PropertyDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // delegated property 的语义由 `typecheck::properties` 覆盖；这里避免引入不完整的 delegate expr typecheck。
    if p.delegate.is_some() {
        return Ok(());
    }

    let Some(init) = &p.init else {
        return Ok(());
    };
    let Some(ty_ref) = &p.ty else {
        // `check_file_headers` 已保证类型注解存在；这里仅做健壮性兜底。
        return Ok(());
    };

    let expected = lower.lower_type_ref(ty_ref)?;

    // initializer 语境的 locals：`this` + 主构造参数。
    let mut locals: HashMap<Span, TypeId> = HashMap::new();
    locals.insert(this_decl_span, this_ty);
    for p in ctor_params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
    }

    let found = infer_expr_type_in_expected_context(
        source,
        init,
        expected,
        ExpectedTypeFrom::new(format!(
            "property `{}` 的类型注解",
            source.slice(p.name.span)
        )),
        lower,
        builtins,
        &locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if is_type_assignable(found, expected, lower, builtins) {
        return Ok(());
    }

    // 与顶层 initializer 一致：允许整数字面量被上下文整数类型吸收（后续可加入 range check）。
    if matches!(init.kind, ast::ExprKind::IntLit) && is_integer_type(expected, lower, builtins) {
        return Ok(());
    }

    Err(ExprTypeError::InitializerTypeMismatch {
        expected: lower.fmt_type(expected),
        found: lower.fmt_type(found),
        span: init.span.into(),
    })
}

/// 检查 class `init { ... }` 初始化块的最小表达式类型（T0448）。
fn check_class_init_block_exprs(
    source: &SourceFile,
    this_decl_span: Span,
    this_ty: TypeId,
    ctor_params: &[ast::Param],
    b: &ast::InitBlockDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let (mut locals, mut stable_bindings, mut mutable_bindings) =
        class_init_locals(this_decl_span, this_ty, ctor_params, lower)?;

    // init block 不是函数体：`return` 在此处无意义，因此 expected_return_ty = None。
    check_block_exprs(
        source,
        &b.body,
        lower,
        builtins,
        &mut locals,
        &mut stable_bindings,
        &mut mutable_bindings,
        0,
        None,
        top_level_types,
        top_level_funs,
        member_mutabilities,
        struct_field_types,
    )?;

    Ok(())
}

/// 检查 class 次构造器 body 的最小表达式类型（T0448）。
fn check_class_secondary_ctor_exprs(
    source: &SourceFile,
    this_decl_span: Span,
    class_fqn: &str,
    has_primary_ctor: bool,
    this_ty: TypeId,
    primary_ctor_params: &[ast::Param],
    ctor: &ast::SecondaryCtorDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // Kotlin-like 语义：当 class 有主构造器时，secondary constructor 必须显式委托到 `this(...)`。
    if has_primary_ctor {
        match ctor.delegation_call.as_ref() {
            None => {
                return Err(ExprTypeError::SecondaryCtorDelegationRequired {
                    class_fqn: class_fqn.to_string(),
                    span: ctor.span.into(),
                });
            }
            Some(call) if call.kind != ast::CtorDelegationKind::This => {
                return Err(ExprTypeError::SecondaryCtorDelegationMustBeThis {
                    class_fqn: class_fqn.to_string(),
                    span: call.target_span.into(),
                });
            }
            Some(_) => {}
        }
    }

    let (mut locals, mut stable_bindings, mut mutable_bindings) =
        class_init_locals(this_decl_span, this_ty, primary_ctor_params, lower)?;

    // 次构造器参数：作为函数参数语义处理（稳定绑定；不可赋值）。
    for p in &ctor.params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
        stable_bindings.insert(p.name.span);
    }

    // secondary ctor body 不是函数体：不允许 `return`。
    check_block_exprs(
        source,
        &ctor.body,
        lower,
        builtins,
        &mut locals,
        &mut stable_bindings,
        &mut mutable_bindings,
        0,
        None,
        top_level_types,
        top_level_funs,
        member_mutabilities,
        struct_field_types,
    )?;

    Ok(())
}

/// 构造 class 初始化语境（property initializer / `init {}` / ctor body）所需的 locals 集合。
///
/// 说明：
/// - `this` 与主构造参数在 resolver 阶段会被写回为 `ResolvedValueRef::Local { decl_span }`；
/// - 这里把这些 decl_span 映射到 TypeId，供后续 type inference 查询。
fn class_init_locals(
    this_decl_span: Span,
    this_ty: TypeId,
    ctor_params: &[ast::Param],
    lower: &mut TypeLowering<'_>,
) -> Result<(HashMap<Span, TypeId>, HashSet<Span>, HashSet<Span>), ExprTypeError> {
    let mut locals: HashMap<Span, TypeId> = HashMap::new();
    let mut stable_bindings: HashSet<Span> = HashSet::new();
    let mutable_bindings: HashSet<Span> = HashSet::new();

    // `this`：resolver 使用 class name 的 span 作为 decl_span。
    locals.insert(this_decl_span, this_ty);
    stable_bindings.insert(this_decl_span);

    // 主构造参数：在初始化语境内可见（T0313）。
    for p in ctor_params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
        stable_bindings.insert(p.name.span);
    }

    Ok((locals, stable_bindings, mutable_bindings))
}

fn check_top_level_val_initializer(
    source: &SourceFile,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let Some(init) = &v.init else {
        return Ok(());
    };
    let Some(ty_ref) = &v.ty else {
        // 顶层 val/var 缺少类型注解会在 `check_file_headers`（T0404）中报错；
        // 这里保持健壮性，不重复报错。
        return Ok(());
    };

    let expected = lower.lower_type_ref(ty_ref)?;
    let expected_from = match &v.binding {
        ast::ValBinding::Name(name) => {
            ExpectedTypeFrom::new(format!("顶层绑定 `{}` 的类型注解", source.slice(name.span)))
        }
        ast::ValBinding::Pattern(_) => ExpectedTypeFrom::new("顶层解构绑定的类型注解"),
    };
    let found = infer_expr_type_in_expected_context(
        source,
        init,
        expected,
        expected_from,
        lower,
        builtins,
        &HashMap::new(),
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if is_type_assignable(found, expected, lower, builtins) {
        return Ok(());
    }

    // 整数字面量（`1`）在静态语义上是“可被上下文整数类型吸收”的常量：
    // - 允许 `val x: UInt8 = 1` 这类写法（后续可在此加入 range check）。
    if matches!(init.kind, ast::ExprKind::IntLit) && is_integer_type(expected, lower, builtins) {
        return Ok(());
    }

    Err(ExprTypeError::InitializerTypeMismatch {
        expected: lower.fmt_type(expected),
        found: lower.fmt_type(found),
        span: init.span.into(),
    })
}

fn unary_op_text(op: ast::UnaryOp) -> &'static str {
    match op {
        ast::UnaryOp::Not => "!",
        ast::UnaryOp::Neg => "-",
        ast::UnaryOp::BitNot => "~",
    }
}

fn binary_op_text(op: ast::BinaryOp) -> &'static str {
    match op {
        ast::BinaryOp::Add => "+",
        ast::BinaryOp::Sub => "-",
        ast::BinaryOp::Mul => "*",
        ast::BinaryOp::Div => "/",
        ast::BinaryOp::Rem => "%",
        ast::BinaryOp::Shl => "<<",
        ast::BinaryOp::Shr => ">>",
        ast::BinaryOp::BitAnd => "&",
        ast::BinaryOp::BitXor => "^",
        ast::BinaryOp::BitOr => "|",
        ast::BinaryOp::Lt => "<",
        ast::BinaryOp::Le => "<=",
        ast::BinaryOp::Gt => ">",
        ast::BinaryOp::Ge => ">=",
        ast::BinaryOp::Eq => "==",
        ast::BinaryOp::Ne => "!=",
        ast::BinaryOp::LogAnd => "&&",
        ast::BinaryOp::LogOr => "||",
        ast::BinaryOp::Elvis => "?:",
    }
}

fn is_integer_type(ty: TypeId, lower: &TypeLowering<'_>, builtins: BuiltinTypes) -> bool {
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

fn unify_integer_operands_for_same_type_rule(
    lhs: &ast::Expr,
    lhs_ty: TypeId,
    rhs: &ast::Expr,
    rhs_ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<TypeId> {
    if lhs_ty == rhs_ty && is_integer_type(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    if matches!(lhs.kind, ast::ExprKind::IntLit) && is_integer_type(rhs_ty, lower, builtins) {
        return Some(rhs_ty);
    }

    if matches!(rhs.kind, ast::ExprKind::IntLit) && is_integer_type(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    None
}

fn infer_unary_expr_type(
    source: &SourceFile,
    op: ast::UnaryOp,
    op_span: Span,
    operand: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let operand_ty = infer_expr_type(
        source,
        operand,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    match op {
        ast::UnaryOp::Not => {
            if operand_ty == builtins.bool_ {
                return Ok(builtins.bool_);
            }

            Err(ExprTypeError::UnaryOpOperandTypeMismatch {
                op: unary_op_text(op).to_string(),
                expected: "Bool".to_string(),
                found: lower.fmt_type(operand_ty),
                span: op_span.into(),
            })
        }
        ast::UnaryOp::Neg | ast::UnaryOp::BitNot => {
            if is_integer_type(operand_ty, lower, builtins) {
                return Ok(operand_ty);
            }

            Err(ExprTypeError::UnaryOpOperandTypeMismatch {
                op: unary_op_text(op).to_string(),
                expected: "整数".to_string(),
                found: lower.fmt_type(operand_ty),
                span: op_span.into(),
            })
        }
    }
}

fn infer_builtin_scalar_binary_expr_type(
    source: &SourceFile,
    lhs: &ast::Expr,
    op: ast::BinaryOp,
    op_span: Span,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let lhs_ty = infer_expr_type(
        source,
        lhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;
    let rhs_ty = infer_expr_type(
        source,
        rhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let mismatch = |expected: &'static str| ExprTypeError::BinaryOpOperandTypeMismatch {
        op: binary_op_text(op).to_string(),
        expected: expected.to_string(),
        lhs: lower.fmt_type(lhs_ty),
        rhs: lower.fmt_type(rhs_ty),
        span: op_span.into(),
    };

    match op {
        // arithmetic: T op T -> T
        ast::BinaryOp::Add
        | ast::BinaryOp::Sub
        | ast::BinaryOp::Mul
        | ast::BinaryOp::Div
        | ast::BinaryOp::Rem
        // bitwise: T op T -> T
        | ast::BinaryOp::BitAnd
        | ast::BinaryOp::BitXor
        | ast::BinaryOp::BitOr => {
            let Some(ty) =
                unify_integer_operands_for_same_type_rule(lhs, lhs_ty, rhs, rhs_ty, lower, builtins)
            else {
                return Err(mismatch("相同的整数类型"));
            };
            Ok(ty)
        }
        // shifts: T << Int -> T
        ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
            if is_integer_type(lhs_ty, lower, builtins) && rhs_ty == builtins.int {
                return Ok(lhs_ty);
            }
            Err(mismatch("lhs 为整数且 rhs 为 Int"))
        }
        // comparisons: T < T -> Bool
        ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
            if unify_integer_operands_for_same_type_rule(lhs, lhs_ty, rhs, rhs_ty, lower, builtins)
                .is_some()
            {
                return Ok(builtins.bool_);
            }
            Err(mismatch("相同的整数类型"))
        }
        // equality: (T == T) -> Bool; (Bool == Bool) -> Bool
        ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
            if lhs_ty == builtins.bool_ && rhs_ty == builtins.bool_ {
                return Ok(builtins.bool_);
            }
            if unify_integer_operands_for_same_type_rule(lhs, lhs_ty, rhs, rhs_ty, lower, builtins)
                .is_some()
            {
                return Ok(builtins.bool_);
            }
            Err(mismatch("相同的整数类型或 Bool"))
        }
        // boolean logic: Bool op Bool -> Bool
        ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
            if lhs_ty == builtins.bool_ && rhs_ty == builtins.bool_ {
                return Ok(builtins.bool_);
            }
            Err(mismatch("Bool"))
        }

        // elvis handled by caller
        ast::BinaryOp::Elvis => Err(ExprTypeError::UnsupportedExpr {
            kind: "elvis expression（internal）",
            span: op_span.into(),
        }),
    }
}

fn infer_expr_type(
    source: &SourceFile,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    match &expr.kind {
        ast::ExprKind::IntLit => Ok(builtins.int),
        ast::ExprKind::StringLit | ast::ExprKind::InterpolatedString { .. } => Ok(builtins.string),
        ast::ExprKind::UnitLit => Ok(builtins.unit),
        ast::ExprKind::Block(b) => infer_block_value_type(
            source,
            b,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::TupleLit { elements } => {
            if elements.is_empty() {
                return Ok(builtins.unit);
            }

            let mut element_types = Vec::with_capacity(elements.len());
            for e in elements {
                element_types.push(infer_expr_type(
                    source,
                    e,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?);
            }

            Ok(lower.ty_tuple(element_types))
        }
        ast::ExprKind::StructLit { ty, fields } => infer_struct_lit_expr_type(
            source,
            expr,
            ty,
            fields,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => infer_if_expr_type(
            source,
            cond.as_ref(),
            then_branch.as_ref(),
            else_branch.as_deref(),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Ident(id) => {
            infer_value_ident_type(source, id, lower, builtins, locals, top_level_types)
        }
        ast::ExprKind::MemberAccess { receiver, member } => infer_member_access_expr_type(
            source,
            receiver.as_ref(),
            member,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::SafeMemberAccess {
            receiver, member, ..
        } => infer_safe_member_access_expr_type(
            source,
            receiver.as_ref(),
            member,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::NotNullAssert {
            expr: inner,
            op_span,
        } => infer_not_null_assert_expr_type(
            source,
            inner.as_ref(),
            *op_span,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Call { callee, args } => infer_call_expr_type(
            source,
            expr,
            callee,
            args,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Cast {
            expr: inner,
            op,
            op_span,
            ty,
        } => {
            let from_ty = infer_expr_type(
                source,
                inner,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            let target_ty = lower.lower_type_ref(ty)?;

            if !is_cast_allowed(from_ty, target_ty, lower, builtins) {
                return Err(ExprTypeError::InvalidCast {
                    from: lower.fmt_type(from_ty),
                    to: lower.fmt_type(target_ty),
                    span: (*op_span).into(),
                });
            }

            match op {
                ast::CastOp::As => {
                    // T0445：`x as T` 的失败语义建模为 `Raise.raise(RuntimeError.ClassCastFailed)`，
                    // 因此在静态 required effects 层面要求 `Raise<RuntimeError>`（除非被 handle/try 捕获）。
                    let runtime_error = lower.lower_type_fqn_with_args(
                        "scoop.core.RuntimeError".to_string(),
                        Vec::new(),
                        *op_span,
                    )?;
                    let raise_runtime_error = lower.lower_type_fqn_with_args(
                        "scoop.core.Raise".to_string(),
                        vec![runtime_error],
                        *op_span,
                    )?;
                    lower.record_performed_effect(raise_runtime_error, *op_span);
                    Ok(target_ty)
                }
                ast::CastOp::AsQ => Ok(lower.ty_option(target_ty)),
            }
        }
        ast::ExprKind::Unary {
            op,
            op_span,
            expr: inner,
        } => infer_unary_expr_type(
            source,
            *op,
            *op_span,
            inner.as_ref(),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::TypeCheck {
            expr: inner, ty, ..
        } => {
            // `is`/`!is` 本身是一个表达式：结果类型为 `Bool`。
            //
            // 当前阶段只做最小检查：
            // - 确保被检查的表达式可推导类型（用于回归覆盖）；
            // - 确保目标类型引用可 lowering（否则应报 type lowering 错误）；
            // - 运行期语义与更强的类型关系约束留到后续阶段（PLAN §4.4 / TODO T0413+）。
            let _ = infer_expr_type(
                source,
                inner,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            let _ = lower.lower_type_ref(ty)?;
            Ok(builtins.bool_)
        }
        ast::ExprKind::When { subject, arms } => {
            // `when` 表达式结果类型：
            // - 递归类型检查 subject 与每个 arm body（保证覆盖其中的表达式）；
            // - 对所有 arm body 的类型做分支合并（T0514：LUB / 受限 union）；
            // - 若所有分支都是 `Nothing`（不可达），则整体结果为 `Nothing`。
            let subject_ty = infer_expr_type(
                source,
                subject,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;

            // 说明：这里必须遍历所有 arm（即使我们已经确定结果会是 `Any`），
            // 以保证：
            // - 分支 body 内的类型错误不会被“短路”吞掉；
            // - 后续的穷尽性检查始终生效。
            let mut result: Option<TypeId> = None;
            for arm in arms {
                // T0427：对 pattern 做最小类型约束，并把 binder 注入到该 arm 的局部环境中。
                let mut arm_locals: HashMap<Span, TypeId> = locals.clone();
                for (decl_span, ty) in when_pat::infer_when_pat_bindings(
                    source, &arm.pat, subject_ty, lower, builtins,
                )? {
                    arm_locals.insert(decl_span, ty);
                }

                // guard：需要在注入 binder 之后检查，这样 `Some(x) if x > 0` 才能在 guard 中引用 `x`。
                if let Some(guard) = &arm.guard {
                    let guard_ty = infer_expr_type(
                        source,
                        guard,
                        lower,
                        builtins,
                        &arm_locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?;
                    if !is_type_assignable(guard_ty, builtins.bool_, lower, builtins) {
                        return Err(ExprTypeError::WhenGuardNotBool {
                            found: lower.fmt_type(guard_ty),
                            span: guard.span.into(),
                        });
                    }
                }

                let arm_ty = infer_expr_type(
                    source,
                    &arm.body,
                    lower,
                    builtins,
                    &arm_locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;

                // `Nothing`：不可达分支（例如后续 `Raise.raise`），不影响分支合并结果。
                if arm_ty == builtins.nothing {
                    continue;
                }

                match result {
                    None => result = Some(arm_ty),
                    Some(prev) => {
                        result = Some(branch_merge::merge_branch_result_type(
                            prev, arm_ty, lower, builtins,
                        ));
                    }
                }
            }

            when_exhaustiveness::check_when_exhaustiveness(
                source, expr, subject_ty, arms, lower, builtins,
            )?;

            // 若所有分支都是 `Nothing`，则 `when` 整体也是不可达的。
            Ok(result.unwrap_or(builtins.nothing))
        }
        ast::ExprKind::Handle {
            body,
            arms,
            finally,
        } => infer_handle_expr_type(
            source,
            expr,
            body,
            arms,
            finally.as_ref(),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Async { body } => infer_async_expr_type(
            source,
            expr,
            body,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Spawn { body } => infer_spawn_expr_type(
            source,
            expr,
            body,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Await { await_span: _, expr: inner } => infer_await_expr_type(
            source,
            expr,
            inner,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Join {
            join_span,
            expr: inner,
        } => infer_join_expr_type(
            source,
            expr,
            *join_span,
            inner,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::WithUpdate { base, updates, .. } => infer_with_update_expr_type(
            source,
            base,
            updates,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Assign { lhs, rhs, .. } => infer_assign_expr_type(
            source,
            lhs,
            rhs,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Binary {
            lhs,
            op,
            op_span,
            rhs,
        } => match op {
            ast::BinaryOp::Elvis => infer_elvis_expr_type(
                source,
                lhs,
                rhs,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            ),
            _ => infer_builtin_scalar_binary_expr_type(
                source,
                lhs.as_ref(),
                *op,
                *op_span,
                rhs.as_ref(),
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            ),
        },
        ast::ExprKind::Lambda(lam) => {
            // T0510：lambda 参数推断失败诊断（最小可读解释）。
            //
            // 说明：
            // - 当前实现只支持“期望函数类型向下传播”的 lambda 推断（T0504）；
            // - 当 lambda 出现在缺少 expected type 的位置（例如 `val f = { x -> x }`）时，
            //   我们给出更明确的错误，而不是笼统的 `unsupported_expr`。
            let Some(param) = lam.params.iter().find(|p| p.ty.is_none()) else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "lambda（当前仅支持在期望函数类型语境下推导）",
                    span: expr.span.into(),
                });
            };

            Err(ExprTypeError::LambdaParamTypeNotInferred {
                param: source.slice(param.name.span).to_string(),
                span: param.name.span.into(),
            })
        }
        ast::ExprKind::Missing => Err(ExprTypeError::UnsupportedExpr {
            kind: "missing",
            span: expr.span.into(),
        }),
        other => Err(ExprTypeError::UnsupportedExpr {
            kind: expr_kind_name(other),
            span: expr.span.into(),
        }),
    }
}

/// 推导 `async { ... }` 的类型，并在 required-effects 收集上“捕获 Async”。
///
/// 当前阶段（T0619）最小规则：
/// - async body 的值类型等价于 block 的值类型；
/// - body 内发生的 `await` 会记录一次 `Async` performed effect；
/// - `async { ... }` 作为语法糖会捕获该 `Async`，因此该 effect 不向外层传播。
fn infer_async_expr_type(
    source: &SourceFile,
    async_expr: &ast::Expr,
    body: &ast::Block,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let async_effect = lower.lower_type_fqn_with_args(
        ASYNC_EFFECT_FQN.to_string(),
        Vec::new(),
        async_expr.span,
    )?;

    let (body_ty, body_performed) = lower.with_nested_effect_collection(|lower| {
        infer_block_value_type(
            source,
            body,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )
    })?;

    // 捕获 async 语境内的 `Async` performed effect（其余 effects 正常向外传播）。
    for (effect, span) in body_performed {
        if effect == async_effect {
            continue;
        }
        lower.record_performed_effect(effect, span);
    }

    Ok(body_ty)
}

/// 推导 `spawn { ... }` 的类型，并把 `Async` 计入 required effects（T0620）。
///
/// 当前阶段（最小可回归落点）：
/// - `spawn` 被视为一次 `Async` performed effect（与规范中 desugar 到 `Async.spawn(...)` 对齐）；
/// - 先只支持 `spawn` body 的值类型为 `Int`，并返回一个 `Int` 句柄（后续由 `Task<T>` 替换）；
/// - 更完整的 `Task<T>` / generic spawn / 取消语义留给后续任务（T0622/T0917）。
fn infer_spawn_expr_type(
    source: &SourceFile,
    spawn_expr: &ast::Expr,
    body: &ast::Block,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let body_ty = infer_block_value_type(
        source,
        body,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let expected_ty = builtins.int;
    if !is_type_assignable(body_ty, expected_ty, lower, builtins) {
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: "spawn".to_string(),
            index: 1,
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(body_ty),
            span: body.span.into(),
        });
    }

    let async_effect = lower.lower_type_fqn_with_args(
        ASYNC_EFFECT_FQN.to_string(),
        Vec::new(),
        spawn_expr.span,
    )?;
    lower.record_performed_effect(async_effect, spawn_expr.span);

    // T0622：为 `spawn/await` 引入 `Task<T>` 的最小类型模型：
    // - 当前阶段仍只支持 `T = Int` 的可执行落点；
    // - `Task<T>` 的运行期语义（lazy/executor/取消）由后续 runtime 任务补齐（T0917）。
    Ok(lower.lower_type_fqn_with_args(
        TASK_FQN.to_string(),
        vec![expected_ty],
        spawn_expr.span,
    )?)
}

fn task_inner_type(ty: TypeId, lower: &TypeLowering<'_>) -> Option<TypeId> {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            if n.fqn == TASK_FQN && n.args.len() == 1 {
                Some(n.args[0])
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 推导 `await expr` 的类型，并把 `Async` 计入 required effects。
///
/// 当前阶段（T0622）最小规则：
/// - `await` 只接受 `Task<T>`，并返回 `T`；
/// - `await` 视为一次 `Async` effect 的 perform 点；
/// - 运行期的 executor/跨线程 resume 语义留给后续任务（T0917+）。
fn infer_await_expr_type(
    source: &SourceFile,
    await_expr: &ast::Expr,
    inner: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let found_ty = infer_expr_type(
        source,
        inner,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let Some(result_ty) = task_inner_type(found_ty, lower) else {
        let expected_task = lower.lower_type_fqn_with_args(
            TASK_FQN.to_string(),
            vec![builtins.any],
            await_expr.span,
        )?;
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: "await".to_string(),
            index: 1,
            expected: lower.fmt_type(expected_task),
            found: lower.fmt_type(found_ty),
            span: inner.span.into(),
        });
    };

    let async_effect = lower.lower_type_fqn_with_args(
        ASYNC_EFFECT_FQN.to_string(),
        Vec::new(),
        await_expr.span,
    )?;
    lower.record_performed_effect(async_effect, await_expr.span);
    Ok(result_ty)
}

/// 推导 `join expr` 的类型，并把 `Async` 计入 required effects（T0620）。
///
/// 当前阶段（最小可回归落点）：
/// - `join` 仅支持等待一个 `Task<T>` 并返回 `T`（当前最小可执行落点仍是 `T = Int`）；
/// - `join` 视为一次 `Async` performed effect（与 `await` 保持一致）。
fn infer_join_expr_type(
    source: &SourceFile,
    _join_expr: &ast::Expr,
    join_span: Span,
    inner: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let found_ty = infer_expr_type(
        source,
        inner,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let Some(result_ty) = task_inner_type(found_ty, lower) else {
        let expected_task = lower.lower_type_fqn_with_args(
            TASK_FQN.to_string(),
            vec![builtins.any],
            join_span,
        )?;
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: "join".to_string(),
            index: 1,
            expected: lower.fmt_type(expected_task),
            found: lower.fmt_type(found_ty),
            span: inner.span.into(),
        });
    };

    let async_effect = lower.lower_type_fqn_with_args(
        ASYNC_EFFECT_FQN.to_string(),
        Vec::new(),
        join_span,
    )?;
    lower.record_performed_effect(async_effect, join_span);

    Ok(result_ty)
}

/// 推导 `block` 作为表达式时的结果类型。
///
/// 说明：
/// - 该入口主要用于 `handle { ... }` 与 handler arm body 的类型检查（T0606）；
/// - 当前实现只覆盖“表达式语境”的最小子集：
///   - 顺序 `val/var` 声明（用于后续语句引用）；
///   - 普通表达式语句（递归调用 `infer_expr_type`），以便记录 required effects；
/// - `return/while/break/continue/comptime` 等语句暂不支持（后续可对齐 `check_block_exprs` 的能力）。
fn infer_block_value_type(
    source: &SourceFile,
    block: &ast::Block,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 与 resolver 的作用域规则对齐：block 内声明仅在该 block 内可见。
    // 这里用“进入时克隆 + 本地更新”的方式实现最小作用域，不要求外层维护 stable/mutable 信息。
    let mut block_locals = locals.clone();
    let mut stable_bindings: HashSet<Span> = HashSet::new();
    let mut mutable_bindings: HashSet<Span> = HashSet::new();

    let mut tail_expr_ty: Option<TypeId> = None;
    for (idx, stmt) in block.stmts.iter().enumerate() {
        let is_last = idx + 1 == block.stmts.len();

        match &stmt.kind {
            ast::StmtKind::Empty => {
                // no-op
            }
            ast::StmtKind::Val(v) => {
                check_local_val_decl_exprs(
                    source,
                    v,
                    lower,
                    builtins,
                    &mut block_locals,
                    &mut stable_bindings,
                    &mut mutable_bindings,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
            }
            ast::StmtKind::Expr(e) => {
                let ty = infer_expr_type(
                    source,
                    e,
                    lower,
                    builtins,
                    &block_locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
                if is_last {
                    tail_expr_ty = Some(ty);
                }
            }
            ast::StmtKind::Missing => {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "block expression（missing stmt）",
                    span: stmt.span.into(),
                });
            }
            _ => {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "block expression（statement kinds other than val/expr）",
                    span: stmt.span.into(),
                });
            }
        }
    }

    Ok(tail_expr_ty.unwrap_or(builtins.unit))
}

/// 推导赋值表达式 `lhs = rhs` 的类型。
///
/// 说明：
/// - AST 中赋值以 `ExprKind::Assign` 承载，但在 HIR 中会降为 `StmtKind::Assign`；
/// - 在 `infer_expr_type` 这条“表达式语境”的入口里，我们缺少 `stable/mutable bindings`
///   信息（它只在 `check_expr_stmt` 的 statement 语境中维护），因此这里先实现最小可回归规则：
///   - lhs 仅允许标识符或成员访问；
///   - rhs 必须可赋给 lhs 的类型（复用 `is_type_assignable`）；
///   - 赋值表达式的结果类型为 `Unit`；
/// - 对“必须是 `var`”的可写性约束，当前阶段仅在 statement 语境（`check_assign_expr_stmt`）
///   中强制；等 `infer_expr_type` 也携带 stable/mutable 后再统一收敛。
fn infer_assign_expr_type(
    source: &SourceFile,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let expected_ty = match &lhs.kind {
        ast::ExprKind::Ident(id) => {
            let Some(resolved) = id.resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（unresolved ident）",
                    span: id.span.into(),
                });
            };

            match resolved {
                ast::ResolvedValueRef::Local { name, decl_span } => locals
                    .get(decl_span)
                    .copied()
                    .ok_or_else(|| ExprTypeError::UnknownLocalValueType {
                        name: name.clone(),
                        span: id.span.into(),
                    })?,
                ast::ResolvedValueRef::TopLevel { .. } => {
                    // 与 statement 语境保持一致：当前阶段先不支持顶层赋值。
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "assignment lhs（top-level value）",
                        span: id.span.into(),
                    });
                }
            }
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            // 先递归 typecheck receiver：保证 `a().b = rhs` 能覆盖 `a()`。
            //
            // 例外：`TypeName.member` 经 companion object 解析时，receiver 不是值表达式；
            // resolver 会保留 receiver ident 为未解析，此处跳过 receiver typecheck。
            let receiver_is_type_name =
                matches!(&receiver.kind, ast::ExprKind::Ident(id) if id.resolved.is_none());
            if !receiver_is_type_name {
                let _ = infer_expr_type(
                    source,
                    receiver,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
            }

            let Some(resolved) = member.resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（member 未 resolve）",
                    span: member.span.into(),
                });
            };

            let fqn = match resolved {
                ast::ResolvedMemberRef::Value { fqn } => fqn,
                ast::ResolvedMemberRef::Fun { fqn }
                | ast::ResolvedMemberRef::ExtensionValue { fqn }
                | ast::ResolvedMemberRef::ExtensionFun { fqn } => {
                    return Err(ExprTypeError::UnsupportedMemberAccess {
                        fqn: fqn.clone(),
                        span: member.span.into(),
                    });
                }
            };

            // 注意：这里不做 member 可写性检查（缺少 member_mutabilities 表）。
            // 若 fqn 不是字段/属性（例如 enum unit variant 值），这里会报 unsupported。
            struct_field_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedMemberAccess {
                    fqn: fqn.clone(),
                    span: member.span.into(),
                }
            })?
        }
        _ => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "assignment lhs（仅支持标识符或成员访问）",
                span: lhs.span.into(),
            });
        }
    };

    // 递归 typecheck rhs：保证 `x = f()` 这类表达式也会覆盖 rhs 中的表达式。
    let expected_from = match &lhs.kind {
        ast::ExprKind::Ident(id) => {
            ExpectedTypeFrom::new(format!("赋值目标 `{}` 的类型", source.slice(id.span)))
        }
        ast::ExprKind::MemberAccess { member, .. } => ExpectedTypeFrom::new(format!(
            "赋值目标 `{}` 的字段类型",
            source.slice(member.span)
        )),
        _ => ExpectedTypeFrom::new("赋值目标的类型"),
    };
    let found_ty = infer_expr_type_in_expected_context(
        source,
        rhs,
        expected_ty,
        expected_from,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
        // 与 initializer/call args 一致：允许整数字面量被上下文整数类型吸收（后续可加入 range check）。
        if matches!(rhs.kind, ast::ExprKind::IntLit) && is_integer_type(expected_ty, lower, builtins)
        {
            return Ok(builtins.unit);
        }
        return Err(ExprTypeError::AssignmentTypeMismatch {
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: rhs.span.into(),
        });
    }

    Ok(builtins.unit)
}

/// 推导 `handle { ... } with { ... }` 表达式的类型，并实现 required effects 的 handler 捕获（T0606）。
///
/// 当前阶段目标（与 TODO T0606 对齐）：
/// - 只支持 non-resuming handler arm（AST 已保证只有 `->` 形态）；
/// - handler arm head 只支持 effect operation（`Effect.op(...)`）；
/// - effect type param 的推断只支持单一 type param（例如 sysroot 的 `Raise<E>`）；
/// - required effects：body 内 perform 的 effect 若被某个 arm 捕获，则不向外层传播。
fn infer_handle_expr_type(
    source: &SourceFile,
    handle_expr: &ast::Expr,
    body: &ast::Block,
    arms: &[ast::HandleArm],
    finally: Option<&ast::Block>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    #[derive(Debug, Clone)]
    struct HandleArmLowered {
        callee_fqn: String,
        handled_effect: TypeId,
        op_return_ty: TypeId,
        binder_tys: Vec<(Span, TypeId)>,
    }

    fn lower_handle_arm_effect_op_sig(
        source: &SourceFile,
        arm: &ast::HandleArm,
        body_performed_effects: &[(TypeId, Span)],
        lower: &mut TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> Result<HandleArmLowered, ExprTypeError> {
        // 1) 解析 effect type 与 op FQN（例如 `scoop.core.Raise.raise`）。
        let effect_fqn = lower.resolve_type_path_fqn(&arm.op.effect)?;
        let op_name = arm.op.op.text(source);
        let callee_fqn = format!("{effect_fqn}.{op_name}");

        // 2) 查找该 member 是否为 effect operation。
        let op = lower.index().by_fqn.get(&callee_fqn).and_then(|syms| {
            syms.fun
                .iter()
                .find(|o| o.sig.kind == ast::FunDeclKind::EffectOp)
                .cloned()
        });
        let Some(op) = op else {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（callee is not an effect operation）",
                span: arm.op.op.span.into(),
            });
        };

        // 3) effect type 必须是 effect。
        let Some(effect_sym) = lower.env().type_symbol(&effect_fqn).cloned() else {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（missing effect type symbol）",
                span: arm.op.effect.span.into(),
            });
        };
        let ok = matches!(
            effect_sym.kind,
            TypeSymbolKind::Nominal(ast::TypeKind::Effect)
        );
        if !ok {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（qualifier is not an effect type）",
                span: arm.op.effect.span.into(),
            });
        }

        // 当前阶段（T0606）只支持单一 type param（与 effect op call 的限制保持一致）。
        if effect_sym.type_param_names.len() > 1 {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（multiple effect type params）",
                span: arm.op.effect.span.into(),
            });
        }
        if op.sig.receiver.is_some() {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（effect op receiver not supported）",
                span: arm.op.op.span.into(),
            });
        }
        if op.sig.type_params_len != 0 {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（generic effect op not supported）",
                span: arm.op.op.span.into(),
            });
        }

        // 4) 构造 effect op 的“可实例化签名”：其参数/返回类型允许引用 effect type 的 type params（例如 `E`）。
        let mut type_params: Vec<TypeId> = Vec::new();
        let mut bindings: Vec<(String, TypeId)> = Vec::new();
        if let Some(name) = effect_sym.type_param_names.first() {
            let param_ty =
                lower.ty_param_named(name.clone(), effect_sym.decl_file.clone(), effect_sym.span);
            type_params.push(param_ty);
            bindings.push((name.clone(), param_ty));
        }

        let mut param_names: Vec<String> = Vec::with_capacity(op.sig.params.len());
        let mut op_params: Vec<TypeId> = Vec::with_capacity(op.sig.params.len());
        for p in &op.sig.params {
            param_names.push(p.name.clone());

            let Some(ty_ref) = p.ty.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "handle arm（effect op param missing type）",
                    span: p.name_span.into(),
                });
            };

            let ty = lower.lower_type_ref_in_decl_file_with_bindings(
                &op.symbol.decl_file,
                bindings.clone(),
                ty_ref,
            )?;
            op_params.push(ty);
        }

        let op_return_ty = match &op.sig.return_ty {
            Some(ret) => lower.lower_type_ref_in_decl_file_with_bindings(
                &op.symbol.decl_file,
                bindings.clone(),
                ret,
            )?,
            None => builtins.unit,
        };

        let param_count = op_params.len();
        let sig = FunSigOwned {
            decl_span: op.symbol.span,
            is_extension: false,
            is_inline: false,
            is_unsafe: false,
            is_nogc: false,
            is_extern: false,
            is_intrinsic: false,
            param_names,
            param_has_defaults: vec![false; param_count],
            type_params: type_params.clone(),
            eff_param: None,
            param_fn_effect_eff_base: vec![None; param_count],
            param_nominal_eff_eff_base: vec![None; param_count],
            param_eff_row_var_subst: vec![EffRowVarSubstPlan::None; param_count],
            return_eff_row_var_subst: EffRowVarSubstPlan::None,
            params: op_params,
            return_ty: op_return_ty,
            effects: None,
        };

        // 5) 决定 effect type args：
        // - 优先使用 handler head 上的显式 type args（`Effect<T>.op(...)`）；
        // - 否则从 binder 的类型注解推断；
        // - 再否则尝试从 handle body 内的 performed effects 反推（仅当唯一候选时）。
        let explicit_args: Vec<TypeId> = arm
            .op
            .effect
            .args
            .iter()
            .map(|a| lower.lower_type_ref(a))
            .collect::<Result<Vec<_>, _>>()?;

        let type_args: Vec<TypeId> = if !explicit_args.is_empty() {
            explicit_args
        } else if type_params.is_empty() {
            Vec::new()
        } else {
            // 先尝试从 binder 的类型注解推断（try/catch lowering 会写回类型注解）。
            let mut constraints: Vec<GenericArgConstraint> = Vec::new();
            for (param_idx, binder) in arm.op.binders.iter().enumerate() {
                let Some(ty_ref) = binder.ty.as_ref() else {
                    continue;
                };
                let binder_ty = lower.lower_type_ref(ty_ref)?;
                constraints.push(GenericArgConstraint {
                    expected: sig.params.get(param_idx).copied().unwrap_or(builtins.unit),
                    found: binder_ty,
                    found_is_placeholder: false,
                    from: format!("handler arm 第 {} 个 binder", param_idx + 1),
                    span: binder.span,
                });
            }

            if !constraints.is_empty() {
                instantiate_fun_sig_for_call(
                    &callee_fqn,
                    arm.span,
                    &sig,
                    constraints,
                    lower,
                    builtins,
                )?
                .type_args
            } else {
                // 没有 binder 类型：尝试从 body 的 performed effects 推断（仅支持“唯一候选”）。
                let mut candidates: Vec<Vec<TypeId>> = Vec::new();
                for (effect, _) in body_performed_effects.iter().copied() {
                    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = lower.type_kind(effect)
                    else {
                        continue;
                    };
                    if nominal.fqn != effect_fqn {
                        continue;
                    }
                    candidates.push(nominal.args);
                }
                candidates.sort();
                candidates.dedup();

                if candidates.len() == 1 {
                    candidates.remove(0)
                } else {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "handle arm（effect type args not inferred）",
                        span: arm.op.effect.span.into(),
                    });
                }
            }
        };

        // 6) 基于 type args 实例化 op 参数类型，并计算 handled effect 的实例类型。
        let instantiated = if !type_params.is_empty() && type_args.len() == type_params.len() {
            let mut params = sig.params.clone();
            let mut return_ty = sig.return_ty;
            for (param_ty, arg_ty) in type_params.iter().copied().zip(type_args.iter().copied()) {
                for p in &mut params {
                    *p = substitute_single_type_param(*p, param_ty, arg_ty, lower, arm.span)?;
                }
                return_ty =
                    substitute_single_type_param(return_ty, param_ty, arg_ty, lower, arm.span)?;
            }
            InstantiatedFunSig {
                params,
                return_ty,
                type_args,
            }
        } else {
            // 无 type params 或者推断失败：退回到未实例化的签名。
            InstantiatedFunSig {
                params: sig.params.clone(),
                return_ty: sig.return_ty,
                type_args,
            }
        };

        let handled_effect =
            lower.lower_type_fqn_with_args(effect_fqn, instantiated.type_args.clone(), arm.span)?;

        // 7) binder 数量校验。
        if arm.op.binders.len() != instantiated.params.len() {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（binder arity mismatch）",
                span: arm.op.span.into(),
            });
        }

        // 8) 校验 binder 类型注解（若存在），并计算 binder 在 arm body 内的类型。
        let mut binder_tys: Vec<(Span, TypeId)> = Vec::with_capacity(arm.op.binders.len());
        for (idx, binder) in arm.op.binders.iter().enumerate() {
            let expected = instantiated.params[idx];

            let binder_ty = match binder.ty.as_ref() {
                Some(ty_ref) => {
                    let binder_ty = lower.lower_type_ref(ty_ref)?;
                    if !is_type_assignable(expected, binder_ty, lower, builtins) {
                        return Err(ExprTypeError::CallArgTypeMismatch {
                            callee: callee_fqn.clone(),
                            index: idx + 1,
                            expected: lower.fmt_type(expected),
                            found: lower.fmt_type(binder_ty),
                            span: binder.span.into(),
                        });
                    }
                    binder_ty
                }
                None => expected,
            };
            binder_tys.push((binder.name.span, binder_ty));
        }

        Ok(HandleArmLowered {
            callee_fqn,
            handled_effect,
            op_return_ty: instantiated.return_ty,
            binder_tys,
        })
    }

    let has_non_resuming = arms
        .iter()
        .any(|a| matches!(a.kind, ast::HandleArmKind::NonResuming));
    let has_immediate_resume = arms.iter().any(|a| {
        matches!(
            a.kind,
            ast::HandleArmKind::ImmediateResume { .. }
        )
    });
    let has_escape_continuation = arms.iter().any(|a| {
        matches!(
            a.kind,
            ast::HandleArmKind::EscapeContinuation { .. }
        )
    });

    if (has_non_resuming && has_immediate_resume)
        || (has_escape_continuation && (has_non_resuming || has_immediate_resume))
    {
        // 早期阶段先拒绝“同一个 handle 中混用 `->` 与 `-> resume`”，避免把结果类型/控制流语义搞混。
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "handle（暂不支持混用 `->` / `-> resume` / `, k ->` arms）",
            span: handle_expr.span.into(),
        });
    }

    // 1) 先在嵌套 effect collection 中 typecheck handle body，
    //    以便：
    //    - 推导 body 的结果类型（用于 handler arm 返回类型一致性检查）
    //    - 收集 performed effects，并在后续根据 handler arms 做过滤（实现 handler 捕获）
    let (body_ty, body_performed) = lower.with_nested_effect_collection(|lower| {
        infer_block_value_type(
            source,
            body,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )
    })?;

    // 2) 处理 handler arms：lower effect op、计算 handled effect 实例、并 typecheck arm bodies。
    // `HandleArm` 的匹配顺序遵循源码中的书写顺序：
    // - 多个 arm 同时可匹配同一个 performed effect 时，选择最先出现的那个；
    // - 若某个 arm 的 handled effect 已被更早的 arm 覆盖，则该 arm 不可达（T0631）。
    let mut handled_effects: Vec<TypeId> = Vec::new();
    let mut seen_by_callee: HashMap<String, Vec<TypeId>> = HashMap::new();

    // handle 表达式的“期望结果类型”：
    // - 若 body 可正常返回（非 Nothing），则 arm body 必须可赋值到该类型；
    // - 若 body 为 Nothing（不可达，例如以 `Raise.raise` 结尾），则以第一个 arm body 的类型作为结果类型。
    let mut result_ty: Option<TypeId> = if has_immediate_resume {
        // `-> resume`：handled computation 会继续执行，因此结果类型固定为 body 的类型。
        Some(body_ty)
    } else if body_ty != builtins.nothing {
        Some(body_ty)
    } else {
        None
    };

    for arm in arms {
        let lowered =
            lower_handle_arm_effect_op_sig(source, arm, &body_performed, lower, builtins)?;

        let seen = seen_by_callee
            .entry(lowered.callee_fqn.clone())
            .or_default();
        if let Some(prev) = seen
            .iter()
            .copied()
            .find(|prev| is_type_assignable(*prev, lowered.handled_effect, lower, builtins))
        {
            return Err(ExprTypeError::HandleArmUnreachable {
                previous: lower.fmt_type(prev),
                current: lower.fmt_type(lowered.handled_effect),
                span: arm.arrow_span.into(),
            });
        }
        seen.push(lowered.handled_effect);
        handled_effects.push(lowered.handled_effect);

        let mut arm_locals = locals.clone();
        for (decl_span, ty) in lowered.binder_tys.iter().copied() {
            arm_locals.insert(decl_span, ty);
        }

        match arm.kind {
            ast::HandleArmKind::ImmediateResume { resume_span } => {
                // `resume(value)`：注入一个局部函数值 `resume: (T) -> Nothing`。
                //
                // 说明：
                // - 当前阶段先用“局部函数值调用”的类型规则复用 call-check；
                // - `resume` 调用的控制流语义由 lowering/codegen（T0616）决定。
                let resume_fun_ty = lower.ty_function(
                    None,
                    vec![lowered.op_return_ty],
                    builtins.unit,
                    EffectRow::pure(),
                );
                arm_locals.insert(resume_span, resume_fun_ty);

                // arm body：只要求可类型检查；不参与 handle 的结果类型推导。
                let _ = infer_expr_type(
                    source,
                    &arm.body,
                    lower,
                    builtins,
                    &arm_locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
            }
            ast::HandleArmKind::EscapeContinuation { k_span } => {
                // `, k ->`：注入 continuation binder 的类型 `Continuation<T>`（T 为 op 返回类型）。
                //
                // 说明：
                // - 当前阶段 continuation 的 effect row 参数仍使用 sysroot 默认值（`Pure`）；
                // - `k.resume(value)` 的 required-effects 传播在 `Continuation.resume` 的内建规则中处理（spec §5.5）。
                let cont_ty = lower.lower_type_fqn_with_args(
                    "scoop.core.Continuation".to_string(),
                    vec![lowered.op_return_ty],
                    arm.span,
                )?;
                arm_locals.insert(k_span, cont_ty);

                // arm body 的类型必须与 handle 的结果类型一致（与 non-resuming 等价：perform 时 handle 立即返回 arm 值）。
                let arm_body_ty = match result_ty {
                    Some(expected) => {
                        let found = infer_expr_type_in_expected_context(
                            source,
                            &arm.body,
                            expected,
                            ExpectedTypeFrom::new("handle 表达式的期望结果类型"),
                            lower,
                            builtins,
                            &arm_locals,
                            top_level_types,
                            top_level_funs,
                            struct_field_types,
                        )?;
                        if !is_type_assignable(found, expected, lower, builtins) {
                            return Err(ExprTypeError::HandleArmReturnTypeMismatch {
                                expected: lower.fmt_type(expected),
                                found: lower.fmt_type(found),
                                span: arm.body.span.into(),
                            });
                        }
                        found
                    }
                    None => {
                        let found = infer_expr_type(
                            source,
                            &arm.body,
                            lower,
                            builtins,
                            &arm_locals,
                            top_level_types,
                            top_level_funs,
                            struct_field_types,
                        )?;
                        result_ty = Some(found);
                        found
                    }
                };

                // 若 body 为 Nothing 且 result_ty 刚刚由该 arm 决定，后续 arms 仍需做一致性校验。
                if let Some(expected) = result_ty {
                    if expected == arm_body_ty {
                        continue;
                    }
                    if is_type_assignable(arm_body_ty, expected, lower, builtins) {
                        continue;
                    }
                    return Err(ExprTypeError::HandleArmReturnTypeMismatch {
                        expected: lower.fmt_type(expected),
                        found: lower.fmt_type(arm_body_ty),
                        span: arm.body.span.into(),
                    });
                }
            }
            ast::HandleArmKind::NonResuming => {
                // arm body 的类型必须与 handle 的结果类型一致（try/catch 等价语义）。
                let arm_body_ty = match result_ty {
                    Some(expected) => {
                        let found = infer_expr_type_in_expected_context(
                            source,
                            &arm.body,
                            expected,
                            ExpectedTypeFrom::new("handle 表达式的期望结果类型"),
                            lower,
                            builtins,
                            &arm_locals,
                            top_level_types,
                            top_level_funs,
                            struct_field_types,
                        )?;
                        if !is_type_assignable(found, expected, lower, builtins) {
                            return Err(ExprTypeError::HandleArmReturnTypeMismatch {
                                expected: lower.fmt_type(expected),
                                found: lower.fmt_type(found),
                                span: arm.body.span.into(),
                            });
                        }
                        found
                    }
                    None => {
                        // body 不可达：用第一个 arm body 的类型作为结果类型。
                        let found = infer_expr_type(
                            source,
                            &arm.body,
                            lower,
                            builtins,
                            &arm_locals,
                            top_level_types,
                            top_level_funs,
                            struct_field_types,
                        )?;
                        result_ty = Some(found);
                        found
                    }
                };

                // 若 body 为 Nothing 且 result_ty 刚刚由该 arm 决定，后续 arms 仍需做一致性校验。
                if let Some(expected) = result_ty {
                    if expected == arm_body_ty {
                        continue;
                    }
                    if is_type_assignable(arm_body_ty, expected, lower, builtins) {
                        continue;
                    }
                    return Err(ExprTypeError::HandleArmReturnTypeMismatch {
                        expected: lower.fmt_type(expected),
                        found: lower.fmt_type(arm_body_ty),
                        span: arm.body.span.into(),
                    });
                }
            }
        }
    }

    // 3) finally block：当前阶段仅递归 typecheck（不参与结果类型），其 performed effects 向外传播。
    if let Some(finally) = finally {
        let _ = infer_block_value_type(
            source,
            finally,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
    }

    // 4) required effects：body 内 performed 的 effects 若被 handler 捕获，则不向外层传播。
    for (effect, span) in body_performed {
        // handler 捕获语义以“可赋值/子类型”为准：若某个 arm 的 handled effect
        // 可以匹配该 performed effect（handled <: performed），则该 effect 不向外传播。
        //
        // 说明：
        // - 对于 invariant effect（或 type args 为 value types 的场景），该关系会退化为全等；
        // - 对于带 `in/out` 的 effect type params，则按声明处变型规则参与匹配。
        let captured = handled_effects
            .iter()
            .copied()
            .any(|handled| is_type_assignable(handled, effect, lower, builtins));
        if captured {
            continue;
        }
        lower.record_performed_effect(effect, span);
    }

    Ok(result_ty.unwrap_or(builtins.nothing))
}

fn infer_if_expr_type(
    source: &SourceFile,
    cond: &ast::Expr,
    then_branch: &ast::Expr,
    else_branch: Option<&ast::Expr>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // `if` 表达式结果类型：
    // - 递归类型检查 cond / then / else（保证覆盖内部表达式）；
    // - then/else 通过 T0514 分支合并规则计算结果类型；
    // - 没有 else 时视为 `Unit`（更接近语句形式）。

    // 先 typecheck cond：保证其中的表达式也会被覆盖（错误不应被吞掉）。
    let _ = infer_expr_type(
        source,
        cond,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // smart cast（T0413）的表达式语境版本（最小实现）：
    // - 与 `check_if_expr_stmt` 保持一致的语义：识别 `if (x is T)` / `if (x !is T)`；
    // - 由于 `infer_expr_type` 当前不携带 stable/mutable bindings 信息，这里采用保守近似：
    //   把当前 `locals` 中出现的绑定视为“可收窄”候选。
    let stable_bindings: HashSet<Span> = locals.keys().copied().collect();
    let smart_cast = detect_smart_cast_for_if_condition(cond, lower, locals, &stable_bindings)?;

    let mut then_locals = locals.clone();
    if let Some(sc) = smart_cast {
        if sc.narrow_in_then {
            then_locals.insert(sc.decl_span, sc.target_ty);
        }
    }
    let then_ty = infer_expr_type(
        source,
        then_branch,
        lower,
        builtins,
        &then_locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let Some(else_branch) = else_branch else {
        // `if` 没有 else：语义上更接近“语句形式”，结果类型视为 `Unit`。
        // 仍然需要确保 then branch 内的表达式被覆盖（见上方 `then_ty`）。
        return Ok(builtins.unit);
    };

    let mut else_locals = locals.clone();
    if let Some(sc) = smart_cast {
        if !sc.narrow_in_then {
            else_locals.insert(sc.decl_span, sc.target_ty);
        }
    }
    let else_ty = infer_expr_type(
        source,
        else_branch,
        lower,
        builtins,
        &else_locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    Ok(branch_merge::merge_branch_result_type(
        then_ty, else_ty, lower, builtins,
    ))
}

/// “期望类型”的来源说明（用于推断失败诊断）。
///
/// 说明：
/// - 该信息会被拼进错误信息的 `Display` 文本，便于 fixtures 用 `EXPECT-ERROR` 做子串断言；
/// - 目前只要求“最小可读解释”，不追求穷尽的来源链路（TODO：后续可扩展为来源栈）。
#[derive(Debug, Clone)]
struct ExpectedTypeFrom {
    desc: String,
}

impl ExpectedTypeFrom {
    fn new(desc: impl Into<String>) -> Self {
        Self { desc: desc.into() }
    }
}

/// 在“存在明确期望类型”的语境下推导表达式类型。
///
/// 目前该入口只做一件事（T0456）：
/// - 当 `Some(x)` 这类 enum variant ctor 在全局存在多个同名候选时，
///   尝试用期望类型（例如 `Option<Int>`）来消歧并继续类型检查；
/// - 若无法消歧，则回退到常规推导逻辑，由原有路径给出稳定的歧义诊断。
fn infer_expr_type_in_expected_context(
    source: &SourceFile,
    expr: &ast::Expr,
    expected_ty: TypeId,
    expected_from: ExpectedTypeFrom,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // T0504：lambda 参数类型下推（spec §14.7.2）。
    //
    // 说明：lambda 的参数类型通常由“期望的函数类型”向下传播而来，因此这里在存在 expected type 时
    // 优先尝试用该信息推断 lambda 的参数类型与返回类型。
    if let ast::ExprKind::Lambda(lam) = &expr.kind {
        if let Some(ty) = try_infer_lambda_expr_type_by_expected(
            source,
            expr,
            lam,
            expected_ty,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )? {
            return Ok(ty);
        }
    }

    // T0510：分支类型不一致时，把推断失败精确映射到具体分支表达式。
    //
    // 说明：
    // - 当前 `if` 表达式的“无 expected type”结果类型推导仍采用 T0503 的最小规则（相同类型否则 Any fallback）；
    // - 但当 `if` 处于“存在明确 expected type”的语境下时，我们可以直接对每个分支做可赋值检查，
    //   并把错误定位到具体分支，而不是让它先退化为 `Any` 再在外层报一个模糊的 mismatch。
    if let ast::ExprKind::If {
        cond,
        then_branch,
        else_branch,
    } = &expr.kind
    {
        let expected_from_desc = expected_from.desc.clone();

        // 先覆盖 cond（不在此处强制 Bool 规则；相关诊断留给控制流/语句层）。
        let _ = infer_expr_type(
            source,
            cond.as_ref(),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        let then_ty = infer_expr_type_in_expected_context(
            source,
            then_branch.as_ref(),
            expected_ty,
            expected_from.clone(),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        if !is_type_assignable(then_ty, expected_ty, lower, builtins)
            && !(matches!(then_branch.kind, ast::ExprKind::IntLit)
                && is_integer_type(expected_ty, lower, builtins))
        {
            return Err(ExprTypeError::IfBranchTypeMismatch {
                branch: "then",
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(then_ty),
                expected_from: expected_from_desc.clone(),
                span: then_branch.span.into(),
            });
        }

        let Some(else_branch) = else_branch.as_deref() else {
            return Ok(builtins.unit);
        };

        let else_ty = infer_expr_type_in_expected_context(
            source,
            else_branch,
            expected_ty,
            expected_from,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        if !is_type_assignable(else_ty, expected_ty, lower, builtins)
            && !(matches!(else_branch.kind, ast::ExprKind::IntLit)
                && is_integer_type(expected_ty, lower, builtins))
        {
            return Err(ExprTypeError::IfBranchTypeMismatch {
                branch: "else",
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(else_ty),
                expected_from: expected_from_desc,
                span: else_branch.span.into(),
            });
        }

        // 两个分支都可赋值给 expected type：直接把整个 `if` 视为该 expected type。
        return Ok(expected_ty);
    }

    if let ast::ExprKind::Call { callee, args } = &expr.kind {
        if let ast::ExprKind::Ident(id) = &callee.kind {
            if id.resolved.is_none() {
                if let Some(ty) = try_infer_ambiguous_enum_variant_ctor_call_expr_type_by_expected(
                    source,
                    expr,
                    id,
                    args,
                    expected_ty,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )? {
                    return Ok(ty);
                }
            }
        }
    }

    infer_expr_type(
        source,
        expr,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )
}

fn try_infer_lambda_expr_type_by_expected(
    source: &SourceFile,
    lam_expr: &ast::Expr,
    lam: &ast::LambdaExpr,
    expected_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let TypeKind::Ref(RefTypeKind::Function(expected_fun)) = lower.type_kind(expected_ty) else {
        return Ok(None);
    };

    // 当前阶段目标（T0504/T0509）：
    // - 支持 0/1 参数 lambda（`() -> T` / `(A) -> T`）
    // - 不支持 receiver function type
    if expected_fun.receiver.is_some() {
        return Ok(None);
    }

    let mut lambda_locals = locals.clone();
    let mut param_tys: Vec<TypeId> = Vec::new();

    match expected_fun.params.len() {
        0 => {
            if !lam.params.is_empty() {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "lambda（当前仅支持 0/1 参数，且参数类型需来自期望函数类型）",
                    span: lam_expr.span.into(),
                });
            }
        }
        1 => {
            if lam.params.len() != 1 {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "lambda（当前仅支持 0/1 参数，且参数类型需来自期望函数类型）",
                    span: lam_expr.span.into(),
                });
            }

            let expected_param_ty = expected_fun.params[0];
            let param = &lam.params[0];
            let param_ty = match &param.ty {
                Some(ty_ref) => lower.lower_type_ref(ty_ref)?,
                None => expected_param_ty,
            };
            lambda_locals.insert(param.name.span, param_ty);
            param_tys.push(param_ty);
        }
        _ => return Ok(None),
    }

    // 返回类型推导（最小）：以 body 表达式的类型为 lambda 返回类型。
    // 当前阶段不做“expected return type 向下传播”（避免引入多段推断链）。
    let (body_ty, performed_effects) = lower.with_nested_effect_collection(|lower| {
        infer_expr_type(
            source,
            lam.body.as_ref(),
            lower,
            builtins,
            &lambda_locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )
    })?;

    let effects = EffectRow::new(
        performed_effects
            .into_iter()
            .map(|(effect, _)| effect)
            .collect(),
    );
    let lam_ty = lower.ty_function(None, param_tys, body_ty, effects);
    Ok(Some(lam_ty))
}

fn try_infer_ambiguous_enum_variant_ctor_call_expr_type_by_expected(
    source: &SourceFile,
    call_expr: &ast::Expr,
    callee: &ast::ValueIdent,
    args: &[ast::Expr],
    expected_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let variant_name = source.slice(callee.span);
    let candidates = lower.env().find_enum_variants_named(variant_name);

    // 只在“同名候选不唯一”的情况下尝试消歧，避免改变原有单候选/无候选的推导与诊断行为。
    if candidates.len() <= 1 {
        return Ok(None);
    }

    let Some((expected_enum_fqn, expected_enum_args)) =
        enum_instance_fqn_and_args_from_type(expected_ty, lower)
    else {
        return Ok(None);
    };

    // 期望类型指向一个明确的 enum：从同名候选中选出“该 enum 的 variant”。
    let mut matched: Vec<(String, EnumVariantInfo)> = candidates
        .into_iter()
        .filter(|(enum_fqn, _)| enum_fqn == &expected_enum_fqn)
        .collect();
    if matched.len() != 1 {
        return Ok(None);
    }
    let (enum_fqn, variant) = matched.pop().expect("len == 1");

    let Some((type_params, enum_source)) = lower.env().enum_decl(&enum_fqn).map(|d| {
        let type_params = d.type_params.clone();
        let source = lower
            .env()
            .source(&d.decl_file)
            .cloned()
            .unwrap_or_else(|| source.clone());
        (type_params, source)
    }) else {
        // 防御性：`matched` 来源于 `TypeEnv.enums`，理论上一定存在。
        return Ok(None);
    };

    if type_params.len() != expected_enum_args.len() {
        // 期望类型与 enum 声明的 arity 不一致时，交给常规推导路径处理并给出诊断。
        return Ok(None);
    }

    let variant_fqn = format!("{enum_fqn}.{variant_name}");
    let expected_arity = variant.fields.len();
    let found_arity = args.len();
    if expected_arity != found_arity {
        return Err(ExprTypeError::EnumVariantCtorArityMismatch {
            variant: variant_fqn,
            expected: expected_arity,
            found: found_arity,
            span: call_expr.span.into(),
        });
    }

    let type_param_set: HashSet<&str> = type_params.iter().map(|s| s.as_str()).collect();
    let subst: HashMap<String, TypeId> = type_params
        .iter()
        .cloned()
        .zip(expected_enum_args.into_iter())
        .collect();

    for (idx, (field, arg_expr)) in variant.fields.iter().zip(args.iter()).enumerate() {
        let expected_field_ty = lower_type_ref_with_enum_subst(
            &enum_source,
            call_expr.span,
            &enum_fqn,
            &field.ty,
            lower,
            builtins,
            &type_param_set,
            &subst,
        )?;

        let found_ty = infer_expr_type_in_expected_context(
            source,
            arg_expr,
            expected_field_ty,
            ExpectedTypeFrom::new(format!(
                "enum variant `{enum_fqn}.{variant_name}` 第 {} 个参数",
                idx + 1
            )),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        if !is_type_assignable(found_ty, expected_field_ty, lower, builtins) {
            return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                variant: format!("{enum_fqn}.{variant_name}"),
                index: idx + 1,
                expected: lower.fmt_type(expected_field_ty),
                found: lower.fmt_type(found_ty),
                span: arg_expr.span.into(),
            });
        }
    }

    Ok(Some(expected_ty))
}

fn enum_instance_fqn_and_args_from_type(
    ty: TypeId,
    lower: &TypeLowering<'_>,
) -> Option<(String, Vec<TypeId>)> {
    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            Some(("scoop.core.Option".to_string(), vec![inner]))
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if matches!(
                lower.nominal_decl_kind(&nominal.fqn),
                Some(ast::TypeKind::Enum)
            ) =>
        {
            Some((nominal.fqn.clone(), nominal.args.clone()))
        }
        _ => None,
    }
}

fn infer_struct_lit_expr_type(
    source: &SourceFile,
    struct_lit_expr: &ast::Expr,
    ty: &ast::TypePath,
    fields: &[ast::StructLitField],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先把 TypeName lowering 为一个 nominal value type（struct/enum）；并进一步约束必须是 struct。
    let struct_ty = lower.lower_type_ref(&ast::TypeRef::Path(ty.clone()))?;

    let (struct_fqn, struct_name) = match lower.type_kind(struct_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            (nominal.fqn, lower.fmt_type(struct_ty))
        }
        _ => {
            return Err(ExprTypeError::StructLitNotStruct {
                found: lower.fmt_type(struct_ty),
                span: ty.span.into(),
            });
        }
    };

    if !matches!(
        lower.nominal_decl_kind(&struct_fqn),
        Some(ast::TypeKind::Struct)
    ) {
        return Err(ExprTypeError::StructLitNotStruct {
            found: struct_name,
            span: ty.span.into(),
        });
    }

    // 收集该 struct 的“直接字段”（不包含 nested type 的字段）。
    //
    // 说明：`collect_struct_field_types` 会为 nested struct 生成形如：
    //   `Outer.Inner.x`
    // 对于 `Outer { ... }` 的 struct literal，我们只接受 `Outer.<field>` 这一层。
    let prefix = format!("{struct_fqn}.");
    let mut expected_fields: HashMap<String, TypeId> = HashMap::new();
    for (field_fqn, field_ty) in struct_field_types {
        let Some(rest) = field_fqn.strip_prefix(&prefix) else {
            continue;
        };
        if rest.contains('.') {
            continue;
        }
        expected_fields.insert(rest.to_string(), *field_ty);
    }

    // 逐项检查：
    // - 字段名不可重复
    // - 字段必须存在于 struct 声明中
    // - 字段初始化表达式类型必须可赋值给字段类型（最小 assignable 规则）
    let mut seen: HashMap<String, Span> = HashMap::new();
    for f in fields {
        let field_name = source.slice(f.name.span).to_string();

        if let Some(prev) = seen.get(&field_name).copied() {
            return Err(ExprTypeError::StructLitDuplicateField {
                struct_name: struct_name.clone(),
                field: field_name,
                first: prev.into(),
                second: f.name.span.into(),
            });
        }
        seen.insert(field_name.clone(), f.name.span);

        let Some(expected_ty) = expected_fields.get(&field_name).copied() else {
            return Err(ExprTypeError::StructLitUnknownField {
                struct_name: struct_name.clone(),
                field: field_name,
                span: f.name.span.into(),
            });
        };

        let found_ty = infer_expr_type_in_expected_context(
            source,
            &f.value,
            expected_ty,
            ExpectedTypeFrom::new(format!(
                "struct `{}` 字段 `{}` 的类型",
                struct_name, field_name
            )),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
            return Err(ExprTypeError::StructLitFieldTypeMismatch {
                struct_name: struct_name.clone(),
                field: field_name,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: f.value.span.into(),
            });
        }
    }

    // 当前阶段（T0423）约束：struct literal 必须显式提供所有字段（不支持默认值/可选字段）。
    let mut missing: Vec<String> = expected_fields
        .keys()
        .filter(|name| !seen.contains_key(*name))
        .cloned()
        .collect();
    missing.sort();
    if !missing.is_empty() {
        // 尽量把错误定位到右花括号 `}`（缺字段通常发生在结尾）。
        let close_brace = if struct_lit_expr.span.end > 0 {
            Span::new(struct_lit_expr.span.end - 1, struct_lit_expr.span.end)
        } else {
            struct_lit_expr.span
        };

        return Err(ExprTypeError::StructLitMissingFields {
            struct_name,
            fields: missing.join(", "),
            span: close_brace.into(),
        });
    }

    Ok(struct_ty)
}

fn infer_with_update_expr_type(
    source: &SourceFile,
    base: &ast::Expr,
    updates: &[ast::WithUpdateField],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先递归类型检查 base：保证 `p with { ... }` 中的 `p` 自身也会被覆盖。
    let base_ty = infer_expr_type(
        source,
        base,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // 当前阶段（T0415）仅支持 struct 字段更新：
    // - base 必须是名义值类型，并且其声明 kind 为 `struct`
    let (base_struct_fqn, base_struct_name) = match lower.type_kind(base_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => (nominal.fqn, lower.fmt_type(base_ty)),
        _ => {
            return Err(ExprTypeError::WithUpdateBaseNotSupported {
                found: lower.fmt_type(base_ty),
                span: base.span.into(),
            });
        }
    };

    if !matches!(
        lower.nominal_decl_kind(&base_struct_fqn),
        Some(ast::TypeKind::Struct)
    ) {
        return Err(ExprTypeError::WithUpdateBaseNotSupported {
            found: base_struct_name,
            span: base.span.into(),
        });
    }

    // `with` 的并行语义：update 之间没有顺序依赖，因此要求：
    // - 完全相同的 path 不能重复出现（否则“谁覆盖谁”会引入顺序）
    // - 一条 path 不能包含另一条 path（例如 `start` 与 `start.x`），否则更新含义不明确
    let mut seen_exact: HashMap<String, Span> = HashMap::new();
    let mut seen_paths: Vec<(Vec<String>, String, Span)> = Vec::new();

    let is_strict_prefix = |a: &[String], b: &[String]| -> bool {
        if a.len() >= b.len() {
            return false;
        }
        a.iter().zip(b.iter()).all(|(x, y)| x == y)
    };

    for u in updates {
        let segments: Vec<String> = u
            .path
            .segments
            .iter()
            .map(|seg| source.slice(seg.span).to_string())
            .collect();
        let path = segments.join(".");

        if let Some(first) = seen_exact.get(&path).copied() {
            return Err(ExprTypeError::WithUpdateDuplicatePath {
                path,
                first: first.into(),
                second: u.path.span.into(),
            });
        }

        for (prev_segments, prev_path, prev_span) in &seen_paths {
            if is_strict_prefix(prev_segments, &segments)
                || is_strict_prefix(&segments, prev_segments)
            {
                // `prev` 与当前 `u` 存在包含关系：报冲突并定位到“第二次出现的那一条”。
                let (parent, child) = if is_strict_prefix(prev_segments, &segments) {
                    (prev_path.clone(), path.clone())
                } else {
                    (path.clone(), prev_path.clone())
                };
                return Err(ExprTypeError::WithUpdateOverlappingPaths {
                    parent,
                    child,
                    first: (*prev_span).into(),
                    second: u.path.span.into(),
                });
            }
        }

        seen_exact.insert(path.clone(), u.path.span);
        seen_paths.push((segments, path, u.path.span));
    }

    for u in updates {
        // 路径可以多段：`a.b.c: value`。
        //
        // 当前阶段限制：
        // - 每一段都必须是 struct 字段
        // - 中间段字段类型必须是 struct（才能继续向下更新）
        let mut current_struct_fqn = base_struct_fqn.clone();
        let mut current_struct_name = lower.fmt_type(base_ty);

        if u.path.segments.is_empty() {
            // parser 不会产生空路径；这里仅保持健壮性。
            return Err(ExprTypeError::WithUpdateNestedPathNotSupported {
                path: "<empty>".to_string(),
                span: u.path.span.into(),
            });
        }

        for (i, seg) in u.path.segments.iter().enumerate() {
            let field = source.slice(seg.span).to_string();
            let field_fqn = format!("{current_struct_fqn}.{field}");
            let Some(field_ty) = struct_field_types.get(&field_fqn).copied() else {
                return Err(ExprTypeError::WithUpdateUnknownField {
                    struct_name: current_struct_name.clone(),
                    field,
                    span: seg.span.into(),
                });
            };

            let is_last = i + 1 == u.path.segments.len();
            if is_last {
                let expected_ty = field_ty;
                let found_ty = infer_expr_type_in_expected_context(
                    source,
                    &u.value,
                    expected_ty,
                    ExpectedTypeFrom::new(format!(
                        "with-update `{}` 字段 `{}` 的类型",
                        current_struct_name, field
                    )),
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;

                if found_ty != expected_ty {
                    return Err(ExprTypeError::WithUpdateFieldTypeMismatch {
                        struct_name: current_struct_name.clone(),
                        field,
                        expected: lower.fmt_type(expected_ty),
                        found: lower.fmt_type(found_ty),
                        span: u.value.span.into(),
                    });
                }

                break;
            }

            // 中间段：必须是 struct 才能继续向下。
            let (next_fqn, next_name) = match lower.type_kind(field_ty) {
                TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                    (nominal.fqn, lower.fmt_type(field_ty))
                }
                _ => {
                    return Err(ExprTypeError::WithUpdateNestedPathNotStruct {
                        struct_name: current_struct_name.clone(),
                        field,
                        found: lower.fmt_type(field_ty),
                        span: seg.span.into(),
                    });
                }
            };

            if !matches!(
                lower.nominal_decl_kind(&next_fqn),
                Some(ast::TypeKind::Struct)
            ) {
                return Err(ExprTypeError::WithUpdateNestedPathNotStruct {
                    struct_name: current_struct_name.clone(),
                    field,
                    found: next_name,
                    span: seg.span.into(),
                });
            }

            current_struct_fqn = next_fqn;
            current_struct_name = next_name;
        }

        // loop 中在最后一段已完成 value typecheck；这里无需额外动作。
    }

    Ok(base_ty)
}

fn is_cast_allowed(
    from: TypeId,
    to: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    if from == to {
        return true;
    }

    // spec §4.4：`as`/`as?` 不做值类型之间的“数值转换”；
    // 但 spec §2.5 允许 value → interface/Any 的显式转换（boxing）。
    //
    // 当前阶段策略：
    // - ref → ref：允许（运行期检查式转换）
    // - value → Any / interface：允许（boxing）
    // - ref → value：不允许（unboxing 需要运行期支持，后续任务补齐）
    if lower.is_ref(from) && lower.is_ref(to) {
        return true;
    }

    // value → Any：允许（boxing）。
    if to == builtins.any && matches!(lower.type_kind(from), TypeKind::Value(_)) {
        return true;
    }

    // value → interface：允许（boxing）。
    match (lower.type_kind(from), lower.type_kind(to)) {
        (
            TypeKind::Value(ValueTypeKind::Nominal(found_nominal)),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            expected_nominal.args.is_empty()
                && expected_nominal.eff.is_none()
                && nominal_is_subtype_by_fqn(&found_nominal.fqn, &expected_nominal.fqn, lower.env())
        }
        _ => false,
    }
}

fn infer_value_ident_type(
    source: &SourceFile,
    id: &ast::ValueIdent,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // `true/false` 当前阶段仍以 ident token 形式存在，但语义上属于字面量。
    let name = source.slice(id.span);
    if name == "true" || name == "false" {
        return Ok(builtins.bool_);
    }

    let Some(resolved) = id.resolved.as_ref() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "ident（未 resolve）",
            span: id.span.into(),
        });
    };

    match resolved {
        ast::ResolvedValueRef::Local { decl_span, .. } => locals
            .get(decl_span)
            .copied()
            .ok_or_else(|| ExprTypeError::UnknownLocalValueType {
                name: name.to_string(),
                span: id.span.into(),
            }),
        ast::ResolvedValueRef::TopLevel { fqn } => {
            if let Some(ty) = top_level_types.get(fqn).copied() {
                return Ok(ty);
            }

            // Kotlin-like：`object Foo` 同时引入一个“类型名 Foo”与一个“值名 Foo”；
            // 在表达式位置引用 `Foo` 时，类型为该 object 的名义类型 `Foo`。
            if lower.is_object_type(fqn) {
                return Ok(lower.lower_type_fqn_with_args(fqn.clone(), Vec::new(), id.span)?);
            }

            Err(ExprTypeError::UnsupportedTopLevelValueType {
                fqn: fqn.clone(),
                span: id.span.into(),
            })
        }
    }
}

#[derive(Debug, Clone)]
enum CallArgKind {
    Positional,
    Named { name: String },
}

#[derive(Debug, Clone)]
struct CallArgInfo<'a> {
    kind: CallArgKind,
    expr: &'a ast::Expr,
    ty: TypeId,
    is_int_lit: bool,
}

/// 把 AST 的调用实参列表归一化为“用于重载筛选”的结构，并预先推导每个实参表达式的类型。
///
/// 说明：
/// - `ExprKind::NamedArg { name = value }` 在调用语境内是“语法糖节点”，其类型应以 `value` 为准；
/// - 这里提前推导所有实参类型，保证：
///   - 子表达式的类型错误不会被重载筛选吞掉；
///   - 后续候选过滤只做纯比较，不再递归进入表达式树。
fn collect_call_arg_infos<'a>(
    source: &SourceFile,
    args: &'a [ast::Expr],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Vec<CallArgInfo<'a>>, ExprTypeError> {
    let mut out: Vec<CallArgInfo<'a>> = Vec::with_capacity(args.len());

    for arg in args {
        match &arg.kind {
            ast::ExprKind::NamedArg { name, value, .. } => {
                let name = source.slice(name.span).to_string();
                let expr = value.as_ref();
                let ty = match expr.kind {
                    // lambda 的类型通常依赖 expected type；在“预收集实参信息”阶段先用占位类型，
                    // 以便后续在“已选定签名”的语境下重新 typecheck（T0504）。
                    ast::ExprKind::Lambda(_) => builtins.any,
                    _ => infer_expr_type(
                        source,
                        expr,
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?,
                };
                out.push(CallArgInfo {
                    kind: CallArgKind::Named { name },
                    expr,
                    ty,
                    is_int_lit: matches!(expr.kind, ast::ExprKind::IntLit),
                });
            }
            _ => {
                let ty = match arg.kind {
                    ast::ExprKind::Lambda(_) => builtins.any,
                    _ => infer_expr_type(
                        source,
                        arg,
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?,
                };
                out.push(CallArgInfo {
                    kind: CallArgKind::Positional,
                    expr: arg,
                    ty,
                    is_int_lit: matches!(arg.kind, ast::ExprKind::IntLit),
                });
            }
        }
    }

    Ok(out)
}

/// 将调用点的“位置/命名实参”映射到某个候选签名的形参槽位。
///
/// 当前阶段（T0453）最小规则：
/// - 不支持默认参数：必须为每个形参提供一个实参；
/// - 命名实参仅按“同名形参”匹配；
/// - 位置实参按从左到右填充尚未被命名实参占用的槽位。
fn map_call_args_to_params(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
) -> Option<Vec<usize>> {
    if call_args.len() != param_names.len() {
        return None;
    }

    let mut mapping: Vec<Option<usize>> = vec![None; param_names.len()];
    let mut next_positional = 0usize;

    for (arg_idx, arg) in call_args.iter().enumerate() {
        match &arg.kind {
            CallArgKind::Positional => {
                while next_positional < mapping.len() && mapping[next_positional].is_some() {
                    next_positional += 1;
                }
                let slot = mapping.get_mut(next_positional)?;
                *slot = Some(arg_idx);
                next_positional += 1;
            }
            CallArgKind::Named { name } => {
                let slot_idx = param_names.iter().position(|p| p == name)?;
                let slot = mapping.get_mut(slot_idx)?;
                if slot.is_some() {
                    return None;
                }
                *slot = Some(arg_idx);
            }
        }
    }

    if mapping.iter().any(|x| x.is_none()) {
        return None;
    }

    Some(mapping.into_iter().map(|x| x.expect("checked")).collect())
}

/// 将调用点的“位置/命名实参”映射到某个候选签名的形参槽位（支持默认参数）。
///
/// 当前阶段（T0454）最小规则：
/// - 允许省略带默认值的形参；
/// - 命名实参仅按“同名形参”匹配；
/// - 位置实参按从左到右填充尚未被命名实参占用的槽位；
/// - 若某个未填充的槽位没有默认值，则该候选不匹配。
fn map_call_args_to_params_with_defaults(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
    param_has_defaults: &[bool],
) -> Option<Vec<Option<usize>>> {
    if param_names.len() != param_has_defaults.len() {
        return None;
    }

    // 默认参数允许“少传”，但不能“多传”。
    if call_args.len() > param_names.len() {
        return None;
    }

    // 最少需要提供的实参数量：无默认值的形参个数。
    let required = param_has_defaults.iter().filter(|d| !**d).count();
    if call_args.len() < required {
        return None;
    }

    let mut mapping: Vec<Option<usize>> = vec![None; param_names.len()];
    let mut next_positional = 0usize;

    for (arg_idx, arg) in call_args.iter().enumerate() {
        match &arg.kind {
            CallArgKind::Positional => {
                while next_positional < mapping.len() && mapping[next_positional].is_some() {
                    next_positional += 1;
                }
                let slot = mapping.get_mut(next_positional)?;
                *slot = Some(arg_idx);
                next_positional += 1;
            }
            CallArgKind::Named { name } => {
                let slot_idx = param_names.iter().position(|p| p == name)?;
                let slot = mapping.get_mut(slot_idx)?;
                if slot.is_some() {
                    return None;
                }
                *slot = Some(arg_idx);
            }
        }
    }

    // 未填充的槽位必须有默认值。
    for (idx, arg_idx) in mapping.iter().copied().enumerate() {
        if arg_idx.is_some() {
            continue;
        }
        if !param_has_defaults.get(idx).copied().unwrap_or(false) {
            return None;
        }
    }

    Some(mapping)
}

fn infer_function_value_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    callee_name: &str,
    callee_decl_span: Span,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 当前阶段（TODO T0710）最小实现：允许调用“局部值中的函数类型”（lambda/闭包/函数值）。
    //
    // 约束：
    // - 暂不支持 receiver function type（`T.(...) -> ...`）；
    // - 暂不支持命名实参（function type 不携带形参名）。
    if args
        .iter()
        .any(|a| matches!(a.kind, ast::ExprKind::NamedArg { .. }))
    {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "函数值调用（暂不支持命名实参）",
            span: call_expr.span.into(),
        });
    }

    let Some(callee_ty) = locals.get(&callee_decl_span).copied() else {
        // 防御性：resolver 已把该引用绑定为 local，但 typecheck locals 未包含该 decl。
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "函数值调用（缺少局部绑定类型信息）",
            span: call_expr.span.into(),
        });
    };

    let TypeKind::Ref(RefTypeKind::Function(fun)) = lower.type_kind(callee_ty) else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    };

    if fun.receiver.is_some() {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "函数值调用（暂不支持 receiver function type）",
            span: call_expr.span.into(),
        });
    }

    let call_args = collect_call_arg_infos(
        source,
        args,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if call_args.len() != fun.params.len() {
        return Err(ExprTypeError::CallArityMismatch {
            callee: callee_name.to_string(),
            expected: fun.params.len(),
            found: call_args.len(),
            span: call_expr.span.into(),
        });
    }

    // 在“期望类型语境”下推导每个实参的最终类型（lambda 会在此处被真正类型检查）。
    let mut checked_arg_tys: Vec<TypeId> = Vec::with_capacity(call_args.len());
    for (idx, arg) in call_args.iter().enumerate() {
        let expected_ty = fun.params[idx];
        let found_ty = infer_expr_type_in_expected_context(
            source,
            arg.expr,
            expected_ty,
            ExpectedTypeFrom::new(format!("函数值 `{callee_name}` 的第 {} 个参数", idx + 1)),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
        checked_arg_tys.push(found_ty);
    }

    // 再做“可赋值”检查（此时 lambda 的 effects 也已经被推断并写入 found_ty）。
    for (idx, (arg, found_ty)) in call_args
        .iter()
        .zip(checked_arg_tys.iter().copied())
        .enumerate()
    {
        let expected_ty = fun.params[idx];
        if is_type_assignable(found_ty, expected_ty, lower, builtins) {
            continue;
        }
        // 整数字面量允许被上下文整数参数类型吸收（后续可加入 range check）。
        if arg.is_int_lit && is_integer_type(expected_ty, lower, builtins) {
            continue;
        }
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: callee_name.to_string(),
            index: idx + 1,
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: arg.expr.span.into(),
        });
    }

    // required effects：调用一个带 effect row 的函数值，需要把该 row 计入当前函数体的 required effects。
    for effect in fun.effects.terms.iter().copied() {
        lower.record_performed_effect(effect, call_expr.span);
    }

    Ok(fun.return_ty)
}

fn collect_top_level_fun_signatures_from_index(
    callee_fqn: &str,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<Vec<FunSigOwned>, ExprTypeError> {
    // 先把 overload 列表复制出来，避免在持有 `lower.index()` 的不可变借用时再调用
    // `lower.lower_type_ref_in_decl_file(...)`（需要可变借用）。
    let overloads = match lower.index().by_fqn.get(callee_fqn) {
        Some(syms) => syms.fun.clone(),
        None => Vec::new(),
    };

    if overloads.is_empty() {
        return Ok(Vec::new());
    }

    // NOTE: `Index` 侧的 `FunSig` 目前只保留了“用于 overload resolution 的声明头信息”，
    // 其中 `type_params_len` 不包含 type param 的名字，因此我们暂时无法在这里把跨文件/跨包的
    // 泛型函数签名完整降低为 `FunSigOwned`（lowering 需要 name→TypeId 绑定）。
    //
    // 为避免误伤未覆盖的用例，这里先只支持非泛型函数签名（`type_params_len == 0`）。
    //
    // 这对于 sysroot 的最小 I/O（`println(String)`）已足够；更完整的跨文件泛型调用将由后续任务补齐。
    let mut out: Vec<FunSigOwned> = Vec::new();
    for o in &overloads {
        if o.sig.type_params_len != 0 {
            continue;
        }

        // `FunSigOwned` 要求“扩展函数 receiver 降糖为第一个参数”；这里与
        // `collect_top_level_fun_signatures` 的约定保持一致。
        let is_extension = o.sig.receiver.is_some();
        // NOTE: `Index::Symbol` 的 `ModifierSet` 当前只保留 override/继承语义所需的少量标记（T0439），
        // 不包含 `inline`。跨文件顶层函数调用暂按 `inline = false` 处理即可（对 println 这类 sysroot API 足够）。
        let is_inline = false;

        // 当前实现只用于“可调用性/参数检查/重载决议”，因此暂不支持跨文件 `eff` 参数与
        // 更复杂的 `E + ...` 替换计划。
        if o.sig.eff_param.is_some() {
            continue;
        }

        let mut param_names = Vec::with_capacity(o.sig.params.len() + usize::from(is_extension));
        let mut param_has_defaults =
            Vec::with_capacity(o.sig.params.len() + usize::from(is_extension));
        let mut params = Vec::with_capacity(o.sig.params.len() + usize::from(is_extension));

        if let Some(receiver) = &o.sig.receiver {
            param_names.push("<receiver>".to_string());
            param_has_defaults.push(false);
            let receiver_ty = lower.lower_type_ref_in_decl_file(&o.symbol.decl_file, receiver)?;
            params.push(receiver_ty);
        }

        for p in &o.sig.params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            param_names.push(p.name.clone());
            param_has_defaults.push(p.has_default);
            let ty = lower.lower_type_ref_in_decl_file(&o.symbol.decl_file, ty_ref)?;
            params.push(ty);
        }

        let return_ty = match &o.sig.return_ty {
            Some(ret) => lower.lower_type_ref_in_decl_file(&o.symbol.decl_file, ret)?,
            None => builtins.unit,
        };

        let param_fn_effect_eff_base: Vec<Option<EffectRow>> = vec![None; params.len()];
        let param_nominal_eff_eff_base: Vec<Option<EffectRow>> = vec![None; params.len()];
        let param_eff_row_var_subst: Vec<EffRowVarSubstPlan> =
            vec![EffRowVarSubstPlan::None; params.len()];

        out.push(FunSigOwned {
            decl_span: o.symbol.span,
            is_extension,
            is_inline,
            is_unsafe: false,
            is_nogc: false,
            is_extern: false,
            is_intrinsic: false,
            param_names,
            param_has_defaults,
            type_params: Vec::new(),
            eff_param: None,
            param_fn_effect_eff_base,
            param_nominal_eff_eff_base,
            param_eff_row_var_subst,
            return_eff_row_var_subst: EffRowVarSubstPlan::None,
            params,
            return_ty,
            effects: o.sig.effects.clone(),
        });
    }

    Ok(out)
}

fn check_unsafe_call_gate(
    callee_fqn: &str,
    sig: &FunSigOwned,
    call_span: Span,
    lower: &TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    if sig.is_extern && !lower.in_unsafe_context() {
        return Err(ExprTypeError::ExternCallRequiresUnsafeContext {
            callee: callee_fqn.to_string(),
            span: call_span.into(),
        });
    }
    if sig.is_unsafe && !lower.in_unsafe_context() {
        return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
            callee: callee_fqn.to_string(),
            span: call_span.into(),
        });
    }
    Ok(())
}

fn infer_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    callee: &ast::Expr,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    match &callee.kind {
        ast::ExprKind::Ident(id) => {
            let callee_name = source.slice(id.span);
            let Some(resolved) = &id.resolved else {
                // T0426：`Some(x)` 这类 enum variant 构造表达式在语法上与普通函数调用一致，
                // 但 resolver 不会把 `Some` 绑定为顶层函数符号，因此这里在“未 resolve 的 ident”
                // 情况下尝试按 enum variant ctor 处理。
                if let Some(ctor_ty) = infer_enum_variant_ctor_call_expr_type(
                    source,
                    call_expr,
                    id,
                    args,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )? {
                    return Ok(ctor_ty);
                }

                // T0454：class 构造调用（primary + secondary constructors）重载决议。
                if let Some(ctor_ty) = infer_class_constructor_call_expr_type(
                    source,
                    call_expr,
                    id,
                    args,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )? {
                    return Ok(ctor_ty);
                }

                return Err(ExprTypeError::CalleeNotCallable {
                    callee: callee_name.to_string(),
                    span: id.span.into(),
                });
            };

            let (callee_fqn, callee_span) = match resolved {
                ast::ResolvedValueRef::TopLevel { fqn } => (fqn.clone(), id.span),
                ast::ResolvedValueRef::Local { decl_span, .. } => {
                    return infer_function_value_call_expr_type(
                        source,
                        call_expr,
                        callee_name,
                        *decl_span,
                        args,
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    );
                }
            };

            // 当前阶段：优先使用“当前文件内”的函数签名信息（支持 return type 推断等回写），
            // 并在缺失时回退到 `Index`（用于 sysroot / 跨文件顶层函数调用）。
            let sigs_from_index: Vec<FunSigOwned>;
            let sigs: &[FunSigOwned] = match top_level_funs.get(&callee_fqn) {
                Some(s) => s.as_slice(),
                None => {
                    sigs_from_index =
                        collect_top_level_fun_signatures_from_index(&callee_fqn, lower, builtins)?;
                    if sigs_from_index.is_empty() {
                        if id.call.is_none() && lower.is_object_type(&callee_fqn) {
                            return Err(ExprTypeError::ObjectNotConstructible {
                                name: callee_fqn,
                                span: callee_span.into(),
                            });
                        }
                        return Err(ExprTypeError::CalleeNotCallable {
                            callee: callee_fqn,
                            span: callee_span.into(),
                        });
                    }
                    sigs_from_index.as_slice()
                }
            };

            // 扩展函数不能以 `f(args...)` 的形式被直接调用，因此这里只选择普通顶层函数候选。
            let direct_call_candidates: Vec<&FunSigOwned> =
                sigs.iter().filter(|s| !s.is_extension).collect();
            let Some(sig) = direct_call_candidates.first().copied() else {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: callee_fqn,
                    span: callee_span.into(),
                });
            };

            // 只有一个可用候选：沿用旧的“给出精确 arity/type mismatch 诊断”的路径，
            // 但补齐命名实参的形参映射（T0453）。
            if direct_call_candidates.len() == 1 {
                check_unsafe_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
                let call_args = collect_call_arg_infos(
                    source,
                    args,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;

                // 默认参数（T0512）：允许省略带默认值的形参。
                //
                // 注意：当前阶段只做“候选可用性/形参映射/类型检查”，不在 AST/HIR 层补齐默认值表达式
                //（默认值补齐语义留给后续任务 T1305）。
                if call_args.len() > sig.params.len() {
                    return Err(ExprTypeError::CallArityMismatch {
                        callee: callee_fqn,
                        expected: sig.params.len(),
                        found: call_args.len(),
                        span: call_expr.span.into(),
                    });
                }

                let required = sig.param_has_defaults.iter().filter(|d| !**d).count();
                if call_args.len() < required {
                    return Err(ExprTypeError::CallArityMismatch {
                        callee: callee_fqn,
                        expected: required,
                        found: call_args.len(),
                        span: call_expr.span.into(),
                    });
                }

                let Some(mapping) = map_call_args_to_params_with_defaults(
                    &call_args,
                    &sig.param_names,
                    &sig.param_has_defaults,
                ) else {
                    return Err(ExprTypeError::NoMatchingOverload {
                        callee: callee_fqn,
                        span: call_expr.span.into(),
                    });
                };

                let mut instantiated = instantiate_fun_sig_for_call(
                    &callee_fqn,
                    call_expr.span,
                    sig,
                    mapping
                        .iter()
                        .copied()
                        .enumerate()
                        .filter_map(|(param_idx, arg_idx)| {
                            let Some(arg_idx) = arg_idx else {
                                return None;
                            };
                            let arg = &call_args[arg_idx];
                            Some(GenericArgConstraint {
                                expected: sig.params[param_idx],
                                found: arg.ty,
                                found_is_placeholder: matches!(
                                    arg.expr.kind,
                                    ast::ExprKind::Lambda(_)
                                ),
                                from: format!("第 {} 个实参", arg_idx + 1),
                                span: arg.expr.span,
                            })
                        }),
                    lower,
                    builtins,
                )?;

                // 先在“期望类型语境”下推导每个实参的最终类型（lambda 会在此处被真正类型检查）。
                let mut checked_arg_tys: Vec<TypeId> = vec![builtins.nothing; call_args.len()];
                for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                    let Some(arg_idx) = arg_idx else {
                        continue;
                    };
                    let arg = &call_args[arg_idx];
                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = infer_expr_type_in_expected_context(
                        source,
                        arg.expr,
                        expected_ty,
                        ExpectedTypeFrom::new(format!(
                            "`{}` 的第 {} 个形参 `{}`",
                            callee_fqn,
                            param_idx + 1,
                            sig.param_names[param_idx]
                        )),
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?;
                    checked_arg_tys[arg_idx] = found_ty;
                }

                // T0509/T0624：推断 `eff` row 参数：
                // - T0509：从 lambda body 的 required effects 推断 `E`；
                // - T0624：从 `Type<eff E>` 形式的实参类型中提取 row 约束（例如 `Disposable<eff Async>`）。
                let eff_arg = if let Some(eff_param) = &sig.eff_param {
                    let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

                    // T0624/T0628a：从 `Type<eff Row>` 的“实参类型”中提取 row 约束。
                    //
                    // 约束形态：`found ⊆ (E + base)`，因此对 `E` 的最小贡献为 `found - base`。
                    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                        let Some(arg_idx) = arg_idx else {
                            continue;
                        };
                        let Some(base) = sig
                            .param_nominal_eff_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };

                        let base = substitute_type_args_in_effect_row(
                            base.clone(),
                            &sig.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        )?;

                        let found_ty = checked_arg_tys[arg_idx];
                        if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                            let delta = effect_row_difference(&found_row, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    // T0509/T0628a：从 lambda body 的 required effects 推断 `E`（同样按 `found - base` 提取增量）。
                    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                        let Some(arg_idx) = arg_idx else {
                            continue;
                        };
                        let Some(base) = sig
                            .param_fn_effect_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };

                        let arg = &call_args[arg_idx];
                        if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                            continue;
                        }

                        let base = substitute_type_args_in_effect_row(
                            base.clone(),
                            &sig.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        )?;

                        let found_ty = checked_arg_tys[arg_idx];
                        if let TypeKind::Ref(RefTypeKind::Function(found_fun)) =
                            lower.type_kind(found_ty)
                        {
                            let delta = effect_row_difference(&found_fun.effects, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    let inferred = EffectRow::new(terms);
                    substitute_type_args_in_effect_row(
                        inferred,
                        &sig.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    )?
                } else {
                    EffectRow::pure()
                };

                instantiate_eff_row_var_in_sig_types(
                    sig,
                    &mut instantiated,
                    &eff_arg,
                    lower,
                    call_expr.span,
                )?;

                // 再做“可赋值”检查（此时 lambda 的 effects 也已经被推断并写入 found_ty）。
                for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                    let Some(arg_idx) = arg_idx else {
                        continue;
                    };
                    let arg = &call_args[arg_idx];
                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = checked_arg_tys[arg_idx];

                    if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
                        // 整数字面量允许被上下文整数参数类型吸收（后续可加入 range check）。
                        if arg.is_int_lit && is_integer_type(expected_ty, lower, builtins) {
                            continue;
                        }
                        return Err(ExprTypeError::CallArgTypeMismatch {
                            callee: callee_fqn,
                            index: param_idx + 1,
                            expected: lower.fmt_type(expected_ty),
                            found: lower.fmt_type(found_ty),
                            span: arg.expr.span.into(),
                        });
                    }
                }

                // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
                let type_param_bindings = type_param_bindings_from_sig(&sig.type_params, lower);
                lower.push_type_param_bindings(type_param_bindings);
                let eff_binding_pushed = if let Some(eff_param) = &sig.eff_param {
                    lower.push_effect_row_param_binding(eff_param.name.clone(), eff_arg.clone());
                    true
                } else {
                    false
                };
                let lowered_effects = lower.lower_effect_row_expr(sig.effects.as_ref());
                if eff_binding_pushed {
                    lower.pop_effect_row_param_binding();
                }
                lower.pop_type_param_bindings();
                let call_effects = substitute_type_args_in_effect_row(
                    lowered_effects?,
                    &sig.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                )?;
                for effect in call_effects.terms.iter().copied() {
                    lower.record_performed_effect(effect, call_expr.span);
                }

                // T0712：记录该泛型函数调用产生的 monomorph key（用于后续生成专用实例）。
                lower.record_monomorph_call(callee_fqn.clone(), sig.decl_span, &instantiated.type_args);

                return Ok(instantiated.return_ty);
            }

            // 多候选：先按形参映射过滤，再对剩余候选尝试泛型/eff 推断（T0512）。
            let call_args = collect_call_arg_infos(
                source,
                args,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;

            #[derive(Debug, Clone)]
            struct MatchedFunOverload<'a> {
                sig: &'a FunSigOwned,
                instantiated: InstantiatedFunSig,
                eff_arg: EffectRow,
                /// `call_args[arg_idx]` 对应的“期望类型”。
                expected_arg_tys: Vec<TypeId>,
                /// 调用点需要用默认值补齐的形参个数（越少越“具体”）。
                defaults_used: usize,
            }

            fn is_strictly_more_specific_fun_overload(
                a: &MatchedFunOverload<'_>,
                b: &MatchedFunOverload<'_>,
                lower: &TypeLowering<'_>,
                builtins: BuiltinTypes,
            ) -> bool {
                let a_le_b = a
                    .expected_arg_tys
                    .iter()
                    .zip(b.expected_arg_tys.iter())
                    .all(|(a_ty, b_ty)| is_type_assignable(*a_ty, *b_ty, lower, builtins));
                let b_le_a = b
                    .expected_arg_tys
                    .iter()
                    .zip(a.expected_arg_tys.iter())
                    .all(|(b_ty, a_ty)| is_type_assignable(*b_ty, *a_ty, lower, builtins));

                a_le_b && !b_le_a
            }

            fn pick_most_specific_fun_overload(
                candidates: &[MatchedFunOverload<'_>],
                lower: &TypeLowering<'_>,
                builtins: BuiltinTypes,
            ) -> Option<usize> {
                // 1) Kotlin-like most-specific：候选 A 的每个形参类型都“更具体”（可赋值到 B 的形参类型），
                //    且至少有一个位置严格更具体，则认为 A 严格更具体。
                for (idx, cand) in candidates.iter().enumerate() {
                    let mut ok = true;
                    for (other_idx, other) in candidates.iter().enumerate() {
                        if idx == other_idx {
                            continue;
                        }
                        if !is_strictly_more_specific_fun_overload(cand, other, lower, builtins) {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        return Some(idx);
                    }
                }

                // 2) tie-break：默认参数更少者优先（“非默认参数优先”）。
                let min_defaults = candidates
                    .iter()
                    .map(|c| c.defaults_used)
                    .min()
                    .unwrap_or(0);
                let mut it = candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.defaults_used == min_defaults);
                let (idx, _) = it.next()?;
                if it.next().is_some() {
                    return None;
                }
                Some(idx)
            }

            let mut matched: Vec<MatchedFunOverload<'_>> = Vec::new();
            for cand in direct_call_candidates {
                let Some(mapping) = map_call_args_to_params_with_defaults(
                    &call_args,
                    &cand.param_names,
                    &cand.param_has_defaults,
                ) else {
                    continue;
                };

                let mut instantiated = match instantiate_fun_sig_for_call(
                    &callee_fqn,
                    call_expr.span,
                    cand,
                    mapping
                        .iter()
                        .copied()
                        .enumerate()
                        .filter_map(|(param_idx, arg_idx)| {
                            let Some(arg_idx) = arg_idx else {
                                return None;
                            };
                            let arg = &call_args[arg_idx];
                            Some(GenericArgConstraint {
                                expected: cand.params[param_idx],
                                found: arg.ty,
                                found_is_placeholder: matches!(
                                    arg.expr.kind,
                                    ast::ExprKind::Lambda(_)
                                ),
                                from: format!("第 {} 个实参", arg_idx + 1),
                                span: arg.expr.span,
                            })
                        }),
                    lower,
                    builtins,
                ) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // 只在需要时（lambda）进入 expected-context typecheck，避免在候选尝试期间把“候选相关”的
                // 副作用（例如调用 required effects）写进外层函数体的 effects 集合。
                let mut ok = true;
                let mut checked_arg_tys: Vec<TypeId> = call_args.iter().map(|a| a.ty).collect();
                for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                    let Some(arg_idx) = arg_idx else {
                        continue;
                    };
                    let arg = &call_args[arg_idx];
                    if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                        continue;
                    }

                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = match infer_expr_type_in_expected_context(
                        source,
                        arg.expr,
                        expected_ty,
                        ExpectedTypeFrom::new(format!(
                            "`{}` 的第 {} 个形参 `{}`",
                            callee_fqn,
                            param_idx + 1,
                            cand.param_names[param_idx]
                        )),
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    ) {
                        Ok(ty) => ty,
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    };
                    checked_arg_tys[arg_idx] = found_ty;
                }
                if !ok {
                    continue;
                }

                // T0509/T0624/T0628a：推断 `eff` row 参数：
                // - 从 lambda body 的 required effects 推断（`found - base`）；
                // - 从 `Type<eff Row>` 形参的实参类型提取 row 约束（`found - base`）。
                let eff_arg = if let Some(eff_param) = &cand.eff_param {
                    let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

                    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                        let Some(arg_idx) = arg_idx else {
                            continue;
                        };
                        let Some(base) = cand
                            .param_nominal_eff_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };

                        let base = match substitute_type_args_in_effect_row(
                            base.clone(),
                            &cand.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        ) {
                            Ok(row) => row,
                            Err(_) => continue,
                        };

                        let found_ty = checked_arg_tys[arg_idx];
                        if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                            let delta = effect_row_difference(&found_row, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                        let Some(arg_idx) = arg_idx else {
                            continue;
                        };
                        let Some(base) = cand
                            .param_fn_effect_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };

                        let arg = &call_args[arg_idx];
                        if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                            continue;
                        }

                        let base = match substitute_type_args_in_effect_row(
                            base.clone(),
                            &cand.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        ) {
                            Ok(row) => row,
                            Err(_) => continue,
                        };

                        let found_ty = checked_arg_tys[arg_idx];
                        if let TypeKind::Ref(RefTypeKind::Function(found_fun)) =
                            lower.type_kind(found_ty)
                        {
                            let delta = effect_row_difference(&found_fun.effects, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    let inferred = EffectRow::new(terms);
                    match substitute_type_args_in_effect_row(
                        inferred,
                        &cand.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    ) {
                        Ok(row) => row,
                        Err(_) => continue,
                    }
                } else {
                    EffectRow::pure()
                };

                if cand.eff_param.is_some()
                    && instantiate_eff_row_var_in_sig_types(
                        cand,
                        &mut instantiated,
                        &eff_arg,
                        lower,
                        call_expr.span,
                    )
                    .is_err()
                {
                    ok = false;
                }
                if !ok {
                    continue;
                }
                for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                    let Some(arg_idx) = arg_idx else {
                        continue;
                    };
                    let arg = &call_args[arg_idx];
                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = checked_arg_tys[arg_idx];

                    if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                        continue;
                    }
                    if arg.is_int_lit && is_integer_type(expected_ty, lower, builtins) {
                        continue;
                    }
                    ok = false;
                    break;
                }

                if ok {
                    let defaults_used = mapping.iter().filter(|x| x.is_none()).count();
                    let mut expected_arg_tys = vec![builtins.nothing; call_args.len()];
                    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                        let Some(arg_idx) = arg_idx else {
                            continue;
                        };
                        expected_arg_tys[arg_idx] = instantiated.params[param_idx];
                    }

                    matched.push(MatchedFunOverload {
                        sig: cand,
                        instantiated,
                        eff_arg,
                        expected_arg_tys,
                        defaults_used,
                    });
                }
            }

            let chosen = match matched.len() {
                0 => {
                    return Err(ExprTypeError::NoMatchingOverload {
                        callee: callee_fqn,
                        span: call_expr.span.into(),
                    });
                }
                1 => matched.pop().expect("len == 1"),
                _ => {
                    let Some(idx) = pick_most_specific_fun_overload(&matched, lower, builtins)
                    else {
                        let name = short_name_from_fqn(&callee_fqn).to_string();
                        let candidates = join_overload_signatures(
                            matched
                                .iter()
                                .map(|c| {
                                    fmt_overload_signature(
                                        &name,
                                        None,
                                        &c.instantiated.params,
                                        lower,
                                    )
                                })
                                .collect(),
                        );
                        return Err(ExprTypeError::AmbiguousOverload {
                            callee: callee_fqn,
                            candidates,
                            span: call_expr.span.into(),
                        });
                    };
                    matched.swap_remove(idx)
                }
            };

            check_unsafe_call_gate(&callee_fqn, chosen.sig, call_expr.span, lower)?;

            // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
            let type_param_bindings = type_param_bindings_from_sig(&chosen.sig.type_params, lower);
            lower.push_type_param_bindings(type_param_bindings);
            let eff_binding_pushed = if let Some(eff_param) = &chosen.sig.eff_param {
                lower.push_effect_row_param_binding(eff_param.name.clone(), chosen.eff_arg.clone());
                true
            } else {
                false
            };
            let lowered_effects = lower.lower_effect_row_expr(chosen.sig.effects.as_ref());
            if eff_binding_pushed {
                lower.pop_effect_row_param_binding();
            }
            lower.pop_type_param_bindings();
            let call_effects = substitute_type_args_in_effect_row(
                lowered_effects?,
                &chosen.sig.type_params,
                &chosen.instantiated.type_args,
                lower,
                call_expr.span,
            )?;
            for effect in call_effects.terms.iter().copied() {
                lower.record_performed_effect(effect, call_expr.span);
            }

            Ok(chosen.instantiated.return_ty)
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            if let Some(ty) = infer_effect_op_call_expr_type(
                source,
                call_expr,
                member,
                args,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )? {
                return Ok(ty);
            }

            infer_member_call_expr_type(
                source,
                call_expr,
                receiver.as_ref(),
                member,
                args,
                false,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )
        }
        ast::ExprKind::SafeMemberAccess {
            receiver, member, ..
        } => infer_member_call_expr_type(
            source,
            call_expr,
            receiver.as_ref(),
            member,
            args,
            true,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        other => Err(ExprTypeError::UnsupportedExpr {
            kind: expr_kind_name(other),
            span: callee.span.into(),
        }),
    }
}

fn is_ctor_visible_from(
    use_cone: ConeId,
    use_source: &SourceFile,
    ctor: &ConstructorOverload,
) -> bool {
    match ctor.visibility {
        Visibility::Public => true,
        Visibility::Internal => ctor.decl_cone == use_cone,
        Visibility::Private => ctor.decl_file.as_path() == use_source.path(),
    }
}

fn infer_class_constructor_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    callee: &ast::ValueIdent,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let Some(call) = callee.call.as_ref() else {
        return Ok(None);
    };

    let mut ctor_types: Vec<String> = call
        .candidates
        .iter()
        .filter_map(|c| match c {
            ast::CallCandidate::Constructor { ty_fqn } => Some(ty_fqn.clone()),
            ast::CallCandidate::Fun { .. } => None,
        })
        .collect();
    ctor_types.sort();
    ctor_types.dedup();

    if ctor_types.is_empty() {
        return Ok(None);
    }

    // T0454 目标：先只覆盖 class constructors（struct literal 的规则独立）。
    ctor_types.retain(|ty_fqn| {
        matches!(
            lower.env().type_symbol(ty_fqn).map(|s| s.kind),
            Some(TypeSymbolKind::Nominal(ast::TypeKind::Class))
        )
    });
    if ctor_types.is_empty() {
        return Ok(None);
    }

    let call_args = collect_call_arg_infos(
        source,
        args,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let use_cone = lower.index().cone_of_source(source);
    let callee_name = source.slice(callee.span).to_string();

    let mut matched: Vec<(String, String)> = Vec::new(); // (type fqn, ctor signature)

    for ty_fqn in ctor_types {
        let Some(ctors) = lower.index().constructors.get(&ty_fqn).cloned() else {
            continue;
        };

        for ctor in ctors
            .iter()
            .filter(|c| is_ctor_visible_from(use_cone, source, c))
        {
            let param_names: Vec<String> = ctor.params.iter().map(|p| p.name.clone()).collect();
            let param_has_defaults: Vec<bool> = ctor.params.iter().map(|p| p.has_default).collect();

            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                continue;
            };

            let mut ok = true;
            for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                let Some(arg_idx) = arg_idx else {
                    continue;
                };

                let Some(expected_ty_ref) = ctor.params.get(param_idx).and_then(|p| p.ty.as_ref())
                else {
                    ok = false;
                    break;
                };
                let expected_ty =
                    lower.lower_type_ref_in_decl_file(&ctor.decl_file, expected_ty_ref)?;

                let arg = &call_args[arg_idx];
                let found_ty = arg.ty;

                if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                    continue;
                }
                if arg.is_int_lit && is_integer_type(expected_ty, lower, builtins) {
                    continue;
                }
                ok = false;
                break;
            }

            if ok {
                let mut param_tys: Vec<String> = Vec::with_capacity(ctor.params.len());
                for p in &ctor.params {
                    let ty =
                        p.ty.as_ref()
                            .map(|t| lower.lower_type_ref_in_decl_file(&ctor.decl_file, t))
                            .transpose()?
                            .map(|ty| lower.fmt_type(ty))
                            .unwrap_or_else(|| "_".to_string());
                    param_tys.push(ty);
                }

                matched.push((
                    ty_fqn.clone(),
                    format!("{ty_fqn}({})", param_tys.join(", ")),
                ));
            }
        }
    }

    match matched.len() {
        0 => Err(ExprTypeError::NoMatchingOverload {
            callee: callee_name,
            span: call_expr.span.into(),
        }),
        1 => {
            let (ty_fqn, _) = matched.pop().expect("len == 1");
            let ty = lower.lower_type_fqn_with_args(ty_fqn, Vec::new(), callee.span)?;
            Ok(Some(ty))
        }
        _ => {
            let candidates =
                join_overload_signatures(matched.into_iter().map(|(_, s)| s).collect());
            Err(ExprTypeError::AmbiguousOverload {
                callee: callee_name,
                candidates,
                span: call_expr.span.into(),
            })
        }
    }
}

fn infer_enum_variant_ctor_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    callee: &ast::ValueIdent,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let variant_name = source.slice(callee.span);
    let candidates = lower.env().find_enum_variants_named(variant_name);

    if candidates.is_empty() {
        return Ok(None);
    }
    if candidates.len() != 1 {
        let mut names: Vec<String> = candidates
            .iter()
            .map(|(enum_fqn, _)| format!("{enum_fqn}.{variant_name}"))
            .collect();
        names.sort();
        names.dedup();

        return Err(ExprTypeError::AmbiguousEnumVariantCtor {
            name: variant_name.to_string(),
            candidates: names.join(" | "),
            span: callee.span.into(),
        });
    }

    let (enum_fqn, variant) = candidates.into_iter().next().expect("len == 1");
    let Some((type_params, enum_source)) = lower.env().enum_decl(&enum_fqn).map(|d| {
        let type_params = d.type_params.clone();
        let source = lower
            .env()
            .source(&d.decl_file)
            .cloned()
            .unwrap_or_else(|| source.clone());
        (type_params, source)
    }) else {
        // 防御性：`candidates` 来源于 `TypeEnv.enums`，理论上一定存在。
        return Ok(None);
    };

    let variant_fqn = format!("{enum_fqn}.{variant_name}");

    let expected = variant.fields.len();
    let found = args.len();
    if expected != found {
        return Err(ExprTypeError::EnumVariantCtorArityMismatch {
            variant: variant_fqn,
            expected,
            found,
            span: call_expr.span.into(),
        });
    }

    // 先推导所有实参类型，保证子表达式（如 `Some(f())`）也会被覆盖。
    let mut arg_types: Vec<TypeId> = Vec::with_capacity(args.len());
    for arg in args {
        arg_types.push(infer_expr_type(
            source,
            arg,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?);
    }

    // enum 的类型参数名集合（用于识别 `T` 这类 type param 引用）。
    let type_param_set: HashSet<&str> = type_params.iter().map(|s| s.as_str()).collect();

    // 早期最小泛型推断（T0426）：
    // - 只从 “payload 字段类型为直接 type param（例如 `T`）” 的位置推断；
    // - 若同一 type param 被多次约束，要求相等（或其中一个为 `Nothing`）。
    let mut subst: HashMap<String, TypeId> = HashMap::new();
    for (idx, (field, found_ty)) in variant
        .fields
        .iter()
        .zip(arg_types.iter().copied())
        .enumerate()
    {
        let ast::TypeRef::Path(p) = &field.ty else {
            continue;
        };
        if !p.args.is_empty() || p.segments.len() != 1 {
            continue;
        }
        let name = enum_source.slice(p.segments[0].span);
        if !type_param_set.contains(name) {
            continue;
        }

        match subst.get(name).copied() {
            None => {
                subst.insert(name.to_string(), found_ty);
            }
            Some(prev) if prev == found_ty => {}
            Some(prev) if prev == builtins.nothing => {
                subst.insert(name.to_string(), found_ty);
            }
            Some(_prev) if found_ty == builtins.nothing => {
                // `Nothing` 不增加额外约束：保留已有推断结果。
            }
            Some(prev) => {
                return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                    variant: format!("{enum_fqn}.{variant_name}"),
                    index: idx + 1,
                    expected: lower.fmt_type(prev),
                    found: lower.fmt_type(found_ty),
                    span: args[idx].span.into(),
                });
            }
        }
    }

    // 逐个检查实参与字段声明类型是否匹配（字段类型允许引用 enum type params）。
    for (idx, (field, found_ty)) in variant
        .fields
        .iter()
        .zip(arg_types.iter().copied())
        .enumerate()
    {
        let expected_ty = lower_type_ref_with_enum_subst(
            &enum_source,
            call_expr.span,
            &enum_fqn,
            &field.ty,
            lower,
            builtins,
            &type_param_set,
            &subst,
        )?;

        if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
            return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                variant: format!("{enum_fqn}.{variant_name}"),
                index: idx + 1,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: args[idx].span.into(),
            });
        }
    }

    // 将推断结果转回 enum 实例类型。
    let mut enum_args: Vec<TypeId> = Vec::with_capacity(type_params.len());
    for name in &type_params {
        let Some(id) = subst.get(name).copied() else {
            return Err(ExprTypeError::EnumVariantCtorTypeArgNotInferred {
                enum_fqn: enum_fqn.clone(),
                param: name.clone(),
                span: callee.span.into(),
            });
        };
        enum_args.push(id);
    }

    let enum_ty = lower.lower_type_fqn_with_args(enum_fqn, enum_args, call_expr.span)?;
    Ok(Some(enum_ty))
}

pub(super) fn lower_type_ref_with_enum_subst(
    enum_source: &SourceFile,
    use_span: Span,
    enum_fqn: &str,
    ty: &ast::TypeRef,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    type_param_set: &HashSet<&str>,
    subst: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    match ty {
        ast::TypeRef::Path(p) => {
            // 单段名且无 type args：可能是对 enum type param 的引用（例如 `T`）。
            if p.segments.len() == 1 && p.args.is_empty() {
                let name = enum_source.slice(p.segments[0].span);
                if type_param_set.contains(name) {
                    return subst.get(name).copied().ok_or_else(|| {
                        ExprTypeError::EnumVariantCtorTypeArgNotInferred {
                            enum_fqn: enum_fqn.to_string(),
                            param: name.to_string(),
                            span: use_span.into(),
                        }
                    });
                }
            }

            let segments: Vec<String> = p
                .segments
                .iter()
                .map(|id| enum_source.slice(id.span).to_string())
                .collect();

            let fqn = match lower.resolve_type_path_fqn_by_name(&segments, use_span) {
                Ok(fqn) => fqn,
                Err(TypeLowerError::UnresolvedType { name, span }) => {
                    let Some(builtin_fqn) = implicit_builtin_type_fqn(&name) else {
                        return Err(TypeLowerError::UnresolvedType { name, span }.into());
                    };
                    builtin_fqn.to_string()
                }
                Err(other) => return Err(other.into()),
            };

            let mut args: Vec<TypeId> = Vec::with_capacity(p.args.len());
            for a in &p.args {
                args.push(lower_type_ref_with_enum_subst(
                    enum_source,
                    use_span,
                    enum_fqn,
                    a,
                    lower,
                    builtins,
                    type_param_set,
                    subst,
                )?);
            }

            Ok(lower.lower_type_fqn_with_args(fqn, args, use_span)?)
        }
        ast::TypeRef::Tuple(t) => {
            if t.elements.is_empty() {
                return Ok(builtins.unit);
            }
            let mut elements: Vec<TypeId> = Vec::with_capacity(t.elements.len());
            for e in &t.elements {
                elements.push(lower_type_ref_with_enum_subst(
                    enum_source,
                    use_span,
                    enum_fqn,
                    e,
                    lower,
                    builtins,
                    type_param_set,
                    subst,
                )?);
            }
            Ok(lower.ty_tuple(elements))
        }
        ast::TypeRef::Nullable { inner, .. } => {
            let inner = lower_type_ref_with_enum_subst(
                enum_source,
                use_span,
                enum_fqn,
                inner,
                lower,
                builtins,
                type_param_set,
                subst,
            )?;
            Ok(lower.ty_option(inner))
        }
        ast::TypeRef::Star { .. } => Err(TypeLowerError::UnsupportedTypeRef {
            kind: "star projection (*)",
            span: use_span.into(),
        }
        .into()),
        ast::TypeRef::EffectRowArg { .. } => Err(TypeLowerError::UnsupportedTypeRef {
            kind: "use-site effect row arg (`eff ...`)",
            span: use_span.into(),
        }
        .into()),
        ast::TypeRef::Function(f) => {
            let receiver = match &f.receiver {
                Some(r) => Some(lower_type_ref_with_enum_subst(
                    enum_source,
                    use_span,
                    enum_fqn,
                    r,
                    lower,
                    builtins,
                    type_param_set,
                    subst,
                )?),
                None => None,
            };

            let mut params = Vec::with_capacity(f.params.len());
            for p in &f.params {
                params.push(lower_type_ref_with_enum_subst(
                    enum_source,
                    use_span,
                    enum_fqn,
                    p,
                    lower,
                    builtins,
                    type_param_set,
                    subst,
                )?);
            }

            let return_ty = lower_type_ref_with_enum_subst(
                enum_source,
                use_span,
                enum_fqn,
                &f.return_ty,
                lower,
                builtins,
                type_param_set,
                subst,
            )?;

            let effects = match &f.effects {
                None => EffectRow::pure(),
                Some(e) if e.terms.is_empty() => EffectRow::pure(),
                Some(e) => {
                    let mut terms: Vec<TypeId> = Vec::with_capacity(e.terms.len());
                    for term in &e.terms {
                        let term_ref = ast::TypeRef::Path(term.clone());
                        let ty = lower_type_ref_with_enum_subst(
                            enum_source,
                            use_span,
                            enum_fqn,
                            &term_ref,
                            lower,
                            builtins,
                            type_param_set,
                            subst,
                        )?;

                        let ok = match lower.type_kind(ty) {
                            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => matches!(
                                lower.nominal_decl_kind(&nominal.fqn),
                                Some(ast::TypeKind::Effect)
                            ),
                            _ => false,
                        };
                        if !ok {
                            return Err(TypeLowerError::EffectRowItemNotEffect {
                                item: enum_source.slice(term.span).to_string(),
                                found: lower.fmt_type(ty),
                                span: term.span.into(),
                            }
                            .into());
                        }

                        terms.push(ty);
                    }
                    EffectRow::new(terms)
                }
            };

            Ok(lower.ty_function(receiver, params, return_ty, effects))
        }
    }
}

fn implicit_builtin_type_fqn(local_or_fqn: &str) -> Option<&'static str> {
    match local_or_fqn {
        // allow both `Int` and `scoop.core.Int` spellings
        "Any" | "scoop.core.Any" => Some("scoop.core.Any"),
        "String" | "scoop.core.String" => Some("scoop.core.String"),
        "Unit" | "scoop.core.Unit" => Some("scoop.core.Unit"),
        "Nothing" | "scoop.core.Nothing" => Some("scoop.core.Nothing"),
        "Bool" | "scoop.core.Bool" => Some("scoop.core.Bool"),
        "Int" | "scoop.core.Int" => Some("scoop.core.Int"),
        "UInt" | "scoop.core.UInt" => Some("scoop.core.UInt"),
        "Option" | "scoop.core.Option" => Some("scoop.core.Option"),
        _ => None,
    }
}

fn infer_effect_op_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let Some(ast::ResolvedMemberRef::Fun { fqn }) = member.resolved.as_ref() else {
        return Ok(None);
    };

    let callee_fqn = fqn.clone();

    // 仅当该 member 解析到一个 effect operation 时，本函数才接管类型检查逻辑；
    // 否则返回 None 让外层继续走 extension/member call 的路径。
    let op = lower.index().by_fqn.get(&callee_fqn).and_then(|syms| {
        syms.fun
            .iter()
            .find(|o| o.sig.kind == ast::FunDeclKind::EffectOp)
            .cloned()
    });
    let Some(op) = op else {
        return Ok(None);
    };

    // effect op 的 qualifier 必须是 effect type（例如 `Raise.raise`），因此这里从 `a.B.op`
    // 反推 effect type FQN 为 `a.B`。
    let Some((effect_ty_fqn, _op_name)) = callee_fqn.rsplit_once('.') else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "effect op call（bad fqn）",
            span: member.span.into(),
        });
    };

    let Some(effect_sym) = lower.env().type_symbol(effect_ty_fqn).cloned() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "effect op call（missing effect type symbol）",
            span: member.span.into(),
        });
    };

    // 当前阶段（T0602）目标：先只支持 sysroot 的 `Raise<E>`（单一 type param），
    // 更完整的 effect polymorphism / 多 type params 留给后续任务（T0609+）。
    if effect_sym.type_param_names.len() > 1 {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "effect op call（multiple effect type params）",
            span: call_expr.span.into(),
        });
    }

    if op.sig.receiver.is_some() {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "effect op call（receiver not supported）",
            span: call_expr.span.into(),
        });
    }

    if op.sig.type_params_len != 0 {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "effect op call（generic op not supported）",
            span: call_expr.span.into(),
        });
    }

    let mut type_params = Vec::new();
    let mut bindings: Vec<(String, TypeId)> = Vec::new();

    if let Some(name) = effect_sym.type_param_names.first() {
        let param_ty =
            lower.ty_param_named(name.clone(), effect_sym.decl_file.clone(), effect_sym.span);
        type_params.push(param_ty);
        bindings.push((name.clone(), param_ty));
    }

    // Lower effect op 签名：参数/返回类型允许引用 effect type 的 type params（例如 `E`）。
    let mut param_names: Vec<String> = Vec::with_capacity(op.sig.params.len());
    let mut params: Vec<TypeId> = Vec::with_capacity(op.sig.params.len());

    for p in &op.sig.params {
        param_names.push(p.name.clone());

        let Some(ty_ref) = p.ty.as_ref() else {
            // headers check 已保证参数类型注解存在；这里保持健壮性。
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "effect op param missing type",
                span: p.name_span.into(),
            });
        };

        let ty = lower.lower_type_ref_in_decl_file_with_bindings(
            &op.symbol.decl_file,
            bindings.clone(),
            ty_ref,
        )?;
        params.push(ty);
    }

    let return_ty = match &op.sig.return_ty {
        Some(ret) => lower.lower_type_ref_in_decl_file_with_bindings(
            &op.symbol.decl_file,
            bindings.clone(),
            ret,
        )?,
        None => builtins.unit,
    };

    let param_count = params.len();
    let sig = FunSigOwned {
        decl_span: op.symbol.span,
        is_extension: false,
        is_inline: false,
        is_unsafe: false,
        is_nogc: false,
        is_extern: false,
        is_intrinsic: false,
        param_names,
        param_has_defaults: vec![false; param_count],
        type_params,
        eff_param: None,
        param_fn_effect_eff_base: vec![None; param_count],
        param_nominal_eff_eff_base: vec![None; param_count],
        param_eff_row_var_subst: vec![EffRowVarSubstPlan::None; param_count],
        return_eff_row_var_subst: EffRowVarSubstPlan::None,
        params,
        return_ty,
        effects: None,
    };

    let call_args = collect_call_arg_infos(
        source,
        args,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if call_args.len() != sig.params.len() {
        return Err(ExprTypeError::CallArityMismatch {
            callee: callee_fqn,
            expected: sig.params.len(),
            found: call_args.len(),
            span: call_expr.span.into(),
        });
    }

    let Some(mapping) = map_call_args_to_params(&call_args, &sig.param_names) else {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_fqn,
            span: call_expr.span.into(),
        });
    };

    let instantiated = instantiate_fun_sig_for_call(
        &callee_fqn,
        call_expr.span,
        &sig,
        mapping
            .iter()
            .copied()
            .enumerate()
            .map(|(param_idx, arg_idx)| {
                let arg = &call_args[arg_idx];
                GenericArgConstraint {
                    expected: sig.params[param_idx],
                    found: arg.ty,
                    found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                    from: format!("第 {} 个实参", arg_idx + 1),
                    span: arg.expr.span,
                }
            }),
        lower,
        builtins,
    )?;

    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
        let arg = &call_args[arg_idx];
        let expected_ty = instantiated.params[param_idx];
        let found_ty = infer_expr_type_in_expected_context(
            source,
            arg.expr,
            expected_ty,
            ExpectedTypeFrom::new(format!(
                "`{}` 的第 {} 个形参 `{}`",
                callee_fqn,
                param_idx + 1,
                sig.param_names[param_idx]
            )),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        if is_type_assignable(found_ty, expected_ty, lower, builtins) {
            continue;
        }
        if arg.is_int_lit && is_integer_type(expected_ty, lower, builtins) {
            continue;
        }

        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: callee_fqn,
            index: param_idx + 1,
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: arg.expr.span.into(),
        });
    }

    // required effects（T0604）：effect op call 视为“立即执行的 perform”，记录到当前函数体的 effects 集合中。
    let effect_instance = lower.lower_type_fqn_with_args(
        effect_ty_fqn.to_string(),
        instantiated.type_args.clone(),
        call_expr.span,
    )?;
    lower.record_performed_effect(effect_instance, call_expr.span);

    Ok(Some(instantiated.return_ty))
}

fn try_infer_continuation_resume_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    receiver_ty: TypeId,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    safe: bool,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Option<TypeId>, ExprTypeError> {
    // spec §5.5：`k.resume(value: T)`。
    //
    // 说明：
    // - 当前阶段 typecheck 尚未支持 class/interface 的实例方法调用；因此这里把 `resume` 视为一个
    //   “内建 member call 形态”，独立于扩展函数解析。
    // - `Continuation<T, eff E>` 的 `E` 视为“调用 resume 可能执行的 required effects”。
    if source.slice(member.span) != "resume" {
        return Ok(None);
    }

    let (expected_value_ty, effects) = match lower.type_kind(receiver_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
            if nominal.fqn == "scoop.core.Continuation" && nominal.args.len() == 1 =>
        {
            (nominal.args[0], nominal.eff.unwrap_or_else(EffectRow::pure))
        }
        _ => return Ok(None),
    };

    let value_expr = match args {
        [] => {
            return Err(ExprTypeError::CallArityMismatch {
                callee: "scoop.core.Continuation.resume".to_string(),
                expected: 1,
                found: 0,
                span: call_expr.span.into(),
            });
        }
        [only] => match &only.kind {
            ast::ExprKind::NamedArg { name, value, .. } => {
                if source.slice(name.span) != "value" {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "Continuation.resume 的命名实参（当前仅支持 `value = ...`）",
                        span: name.span.into(),
                    });
                }
                value.as_ref()
            }
            _ => only,
        },
        _ => {
            return Err(ExprTypeError::CallArityMismatch {
                callee: "scoop.core.Continuation.resume".to_string(),
                expected: 1,
                found: args.len(),
                span: call_expr.span.into(),
            });
        }
    };

    let found_value_ty = infer_expr_type(
        source,
        value_expr,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if !is_type_assignable(found_value_ty, expected_value_ty, lower, builtins) {
        if matches!(value_expr.kind, ast::ExprKind::IntLit)
            && is_integer_type(expected_value_ty, lower, builtins)
        {
            // 整数字面量允许被上下文整数类型吸收（与普通调用保持一致）。
        } else {
            return Err(ExprTypeError::CallArgTypeMismatch {
                callee: "scoop.core.Continuation.resume".to_string(),
                index: 1,
                expected: lower.fmt_type(expected_value_ty),
                found: lower.fmt_type(found_value_ty),
                span: value_expr.span.into(),
            });
        }
    }

    // required effects：`resume` 视为“立即执行 continuation 的下一步”，因此把 `E` 计入当前函数体的 required effects。
    for effect in effects.terms.iter().copied() {
        lower.record_performed_effect(effect, call_expr.span);
    }

    let ret = if safe {
        lower.ty_option(builtins.unit)
    } else {
        builtins.unit
    };
    Ok(Some(ret))
}

fn infer_member_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    safe: bool,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先递归类型检查 receiver：保证 `a?.b()` 中的 `a` 自身也会被覆盖。
    let receiver_ty = infer_expr_type(
        source,
        receiver,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let actual_receiver_ty = if safe {
        match lower.type_kind(receiver_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
            _ => {
                return Err(ExprTypeError::SafeAccessReceiverNotNullable {
                    found: lower.fmt_type(receiver_ty),
                    span: receiver.span.into(),
                });
            }
        }
    } else {
        receiver_ty
    };

    if let Some(ret) = try_infer_continuation_resume_call_expr_type(
        source,
        call_expr,
        actual_receiver_ty,
        member,
        args,
        safe,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )? {
        return Ok(ret);
    }

    // spec §8.4：`String.trimIndent()` 是内建的 `const fun`（运行期可回退为普通调用）。
    //
    // 说明：
    // - 早期阶段 `String` API 尚未完整通过 sysroot 声明并接入“扩展函数调用”路径；
    // - 这里先以 intrinsic 的形式固定最小类型规则：`String.trimIndent(): String`。
    //
    // TODO T1216：接入编译期求值（当 receiver 为编译期常量时折叠）。
    let member_name = source.slice(member.span);
    if member_name == "trimIndent" && actual_receiver_ty == builtins.string {
        if !args.is_empty() {
            return Err(ExprTypeError::CallArityMismatch {
                callee: "trimIndent".to_string(),
                expected: 0,
                found: args.len(),
                span: call_expr.span.into(),
            });
        }
        return Ok(builtins.string);
    }

    // 当前阶段只支持“扩展函数调用”（T0312）：`receiver.member(args...)`。
    // - 若 resolver 已写回 `ExtensionFun`，优先使用；
    // - 否则（例如 `receiver` 为 `T?` 时 resolver 无法静态确定 receiver 类型），
    //   尝试在“当前包”内按同名顶层 fun 查找 receiver fun。
    let callee_fqn = match member.resolved.as_ref() {
        Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) => fqn.clone(),
        Some(ast::ResolvedMemberRef::Fun { fqn })
        | Some(ast::ResolvedMemberRef::Value { fqn })
        | Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) => {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: fqn.clone(),
                span: member.span.into(),
            });
        }
        None => {
            let name = source.slice(member.span);
            if lower.pkg_prefix().is_empty() {
                name.to_string()
            } else {
                format!("{}.{}", lower.pkg_prefix(), name)
            }
        }
    };

    let Some(sigs) = top_level_funs.get(&callee_fqn) else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: member.span.into(),
        });
    };

    // 只选择扩展函数候选（同名顶层普通函数不参与 `receiver.member()`）。
    let ext_candidates: Vec<&FunSigOwned> = sigs.iter().filter(|s| s.is_extension).collect();
    let Some(sig) = ext_candidates.first().copied() else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: member.span.into(),
        });
    };

    // 预先推导所有“显式实参”的类型（不含 receiver），并归一化 named arg 的语法糖节点，
    // 以便在重载筛选中复用这份结果并避免把子表达式错误吞掉。
    let call_args = collect_call_arg_infos(
        source,
        args,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let Some(expected_receiver_ty) = sig.params.first().copied() else {
        // 健壮性：扩展函数至少应该包含 receiver 这一参数。
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: member.span.into(),
        });
    };

    // 只有一个扩展候选：沿用旧的“给出精确 mismatch 诊断”的路径，但补齐命名实参映射（T0453）。
    if ext_candidates.len() == 1 {
        check_unsafe_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
        let expected_args = sig.params.len().saturating_sub(1);
        if call_args.len() > expected_args {
            return Err(ExprTypeError::CallArityMismatch {
                callee: callee_fqn,
                expected: expected_args,
                found: call_args.len(),
                span: call_expr.span.into(),
            });
        }

        let Some(param_names) = sig.param_names.get(1..) else {
            // 健壮性：扩展函数至少应该包含 receiver 的占位形参名。
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };
        let Some(param_has_defaults) = sig.param_has_defaults.get(1..) else {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };

        let required = param_has_defaults.iter().filter(|d| !**d).count();
        if call_args.len() < required {
            return Err(ExprTypeError::CallArityMismatch {
                callee: callee_fqn,
                expected: required,
                found: call_args.len(),
                span: call_expr.span.into(),
            });
        }

        let Some(mapping) =
            map_call_args_to_params_with_defaults(&call_args, param_names, param_has_defaults)
        else {
            return Err(ExprTypeError::NoMatchingOverload {
                callee: callee_fqn,
                span: call_expr.span.into(),
            });
        };

        let mut instantiated = instantiate_fun_sig_for_call(
            &callee_fqn,
            call_expr.span,
            sig,
            std::iter::once(GenericArgConstraint {
                expected: expected_receiver_ty,
                found: actual_receiver_ty,
                found_is_placeholder: false,
                from: "接收者（receiver）".to_string(),
                span: receiver.span,
            })
            .chain(mapping.iter().copied().enumerate().filter_map(
                |(param_idx, arg_idx)| {
                    let Some(arg_idx) = arg_idx else {
                        return None;
                    };
                    let arg = &call_args[arg_idx];
                    Some(GenericArgConstraint {
                        expected: sig.params[param_idx + 1],
                        found: arg.ty,
                        found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                        from: format!("第 {} 个实参", arg_idx + 1),
                        span: arg.expr.span,
                    })
                },
            )),
            lower,
            builtins,
        )?;

        // receiver mismatch 检查：
        // - 默认路径：在推断 `eff` row 参数之前就可以做 receiver 可赋值检查，给出更精确诊断；
        // - 但当 receiver 的期望类型依赖 `E`（例如 `Type<eff (E + IO)>`，或更深的嵌套位置）时，
        //   receiver 的“期望类型”必须等到 `E` 被实例化后才能确定（T0624）。
        let receiver_uses_eff = sig.eff_param.is_some()
            && sig
                .param_eff_row_var_subst
                .get(0)
                .is_some_and(|p| p.uses_eff_var());
        if !receiver_uses_eff {
            let expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(expected_receiver_ty);
            if !is_type_assignable(actual_receiver_ty, expected_receiver_ty, lower, builtins) {
                return Err(ExprTypeError::CallReceiverTypeMismatch {
                    callee: callee_fqn,
                    expected: lower.fmt_type(expected_receiver_ty),
                    found: lower.fmt_type(actual_receiver_ty),
                    span: receiver.span.into(),
                });
            }
        }

        // 先在“期望类型语境”下推导每个显式实参的最终类型（lambda 会在此处被真正类型检查）。
        let mut checked_arg_tys: Vec<TypeId> = vec![builtins.nothing; call_args.len()];
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &call_args[arg_idx];
            let found_ty = infer_expr_type_in_expected_context(
                source,
                arg.expr,
                expected_ty,
                ExpectedTypeFrom::new(format!(
                    "`{}` 的第 {} 个形参 `{}`",
                    callee_fqn,
                    param_idx + 2,
                    sig.param_names[param_idx + 1]
                )),
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            checked_arg_tys[arg_idx] = found_ty;
        }

        // T0509/T0624/T0628a：推断 `eff` row 参数：
        // - 从 lambda body 的 required effects 推断（`found - base`）；
        // - 从 `Type<eff Row>` receiver/形参的实参类型提取 row 约束（`found - base`）。
        let eff_arg = if let Some(eff_param) = &sig.eff_param {
            let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

            // receiver 约束：`ReceiverType<eff Row>`。
            if let Some(base) = sig
                .param_nominal_eff_eff_base
                .get(0)
                .and_then(|b| b.as_ref())
            {
                let base = substitute_type_args_in_effect_row(
                    base.clone(),
                    &sig.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                )?;
                if let Some(found_row) = nominal_eff_row_from_type(actual_receiver_ty, lower) {
                    let delta = effect_row_difference(&found_row, &base);
                    terms.extend(delta.terms);
                }
            }

            for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                let Some(arg_idx) = arg_idx else {
                    continue;
                };
                let sig_param_idx = param_idx + 1; // 跳过 receiver

                // `Type<eff Row>` 形参约束。
                if let Some(base) = sig
                    .param_nominal_eff_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                {
                    let base = substitute_type_args_in_effect_row(
                        base.clone(),
                        &sig.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    )?;
                    let found_ty = checked_arg_tys[arg_idx];
                    if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                        let delta = effect_row_difference(&found_row, &base);
                        terms.extend(delta.terms);
                    }
                }

                let Some(base) = sig
                    .param_fn_effect_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                else {
                    continue;
                };
                let arg = &call_args[arg_idx];
                if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                    continue;
                }

                let base = substitute_type_args_in_effect_row(
                    base.clone(),
                    &sig.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                )?;
                let found_ty = checked_arg_tys[arg_idx];
                if let TypeKind::Ref(RefTypeKind::Function(found_fun)) = lower.type_kind(found_ty) {
                    let delta = effect_row_difference(&found_fun.effects, &base);
                    terms.extend(delta.terms);
                }
            }

            let inferred = EffectRow::new(terms);
            substitute_type_args_in_effect_row(
                inferred,
                &sig.type_params,
                &instantiated.type_args,
                lower,
                call_expr.span,
            )?
        } else {
            EffectRow::pure()
        };

        instantiate_eff_row_var_in_sig_types(
            sig,
            &mut instantiated,
            &eff_arg,
            lower,
            call_expr.span,
        )?;

        // 若 receiver 依赖 `E`，现在 `E` 已实例化完毕，补做 receiver mismatch 检查。
        if receiver_uses_eff {
            let expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(expected_receiver_ty);
            if !is_type_assignable(actual_receiver_ty, expected_receiver_ty, lower, builtins) {
                return Err(ExprTypeError::CallReceiverTypeMismatch {
                    callee: callee_fqn,
                    expected: lower.fmt_type(expected_receiver_ty),
                    found: lower.fmt_type(actual_receiver_ty),
                    span: receiver.span.into(),
                });
            }
        }

        // 再做“可赋值”检查（此时 lambda 的 effects 也已经被推断并写入 found_ty）。
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &call_args[arg_idx];
            let found_ty = checked_arg_tys[arg_idx];

            if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                continue;
            }
            if arg.is_int_lit && is_integer_type(expected_ty, lower, builtins) {
                continue;
            }

            return Err(ExprTypeError::CallArgTypeMismatch {
                callee: callee_fqn,
                // extension 调用：`receiver.member(arg1, arg2, ...)` 的第 1 个“显式参数”
                // 对应 `sig.params[1]`（跳过 receiver 参数）。
                index: param_idx + 1,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: arg.expr.span.into(),
            });
        }

        // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
        let type_param_bindings = type_param_bindings_from_sig(&sig.type_params, lower);
        lower.push_type_param_bindings(type_param_bindings);
        let eff_binding_pushed = if let Some(eff_param) = &sig.eff_param {
            lower.push_effect_row_param_binding(eff_param.name.clone(), eff_arg.clone());
            true
        } else {
            false
        };
        let lowered_effects = lower.lower_effect_row_expr(sig.effects.as_ref());
        if eff_binding_pushed {
            lower.pop_effect_row_param_binding();
        }
        lower.pop_type_param_bindings();
        let call_effects = substitute_type_args_in_effect_row(
            lowered_effects?,
            &sig.type_params,
            &instantiated.type_args,
            lower,
            call_expr.span,
        )?;
        for effect in call_effects.terms.iter().copied() {
            lower.record_performed_effect(effect, call_expr.span);
        }

        let ret = if safe {
            lower.ty_option(instantiated.return_ty)
        } else {
            instantiated.return_ty
        };

        return Ok(ret);
    }

    #[derive(Debug, Clone)]
    struct MatchedExtensionOverload<'a> {
        sig: &'a FunSigOwned,
        instantiated: InstantiatedFunSig,
        eff_arg: EffectRow,
        receiver_ty: TypeId,
        /// `call_args[arg_idx]` 对应的“期望类型”（排除了 receiver 参数）。
        expected_arg_tys: Vec<TypeId>,
        /// 调用点需要用默认值补齐的形参个数（越少越“具体”）。
        defaults_used: usize,
    }

    fn is_strictly_more_specific_extension_overload(
        a: &MatchedExtensionOverload<'_>,
        b: &MatchedExtensionOverload<'_>,
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> bool {
        let a_le_b = is_type_assignable(a.receiver_ty, b.receiver_ty, lower, builtins)
            && a.expected_arg_tys
                .iter()
                .zip(b.expected_arg_tys.iter())
                .all(|(a_ty, b_ty)| is_type_assignable(*a_ty, *b_ty, lower, builtins));
        let b_le_a = is_type_assignable(b.receiver_ty, a.receiver_ty, lower, builtins)
            && b.expected_arg_tys
                .iter()
                .zip(a.expected_arg_tys.iter())
                .all(|(b_ty, a_ty)| is_type_assignable(*b_ty, *a_ty, lower, builtins));

        a_le_b && !b_le_a
    }

    fn pick_most_specific_extension_overload(
        candidates: &[MatchedExtensionOverload<'_>],
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> Option<usize> {
        for (idx, cand) in candidates.iter().enumerate() {
            let mut ok = true;
            for (other_idx, other) in candidates.iter().enumerate() {
                if idx == other_idx {
                    continue;
                }
                if !is_strictly_more_specific_extension_overload(cand, other, lower, builtins) {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some(idx);
            }
        }

        // tie-break：默认参数更少者优先（“非默认参数优先”）。
        let min_defaults = candidates
            .iter()
            .map(|c| c.defaults_used)
            .min()
            .unwrap_or(0);
        let mut it = candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.defaults_used == min_defaults);
        let (idx, _) = it.next()?;
        if it.next().is_some() {
            return None;
        }
        Some(idx)
    }

    // 多候选：先按 receiver/参数匹配筛选，再用 receiver/参数 specificity 选出 most-specific（T0455）。
    let mut matched: Vec<MatchedExtensionOverload<'_>> = Vec::new();

    for cand in ext_candidates {
        let Some(param_names) = cand.param_names.get(1..) else {
            continue;
        };
        let Some(param_has_defaults) = cand.param_has_defaults.get(1..) else {
            continue;
        };
        let Some(mapping) =
            map_call_args_to_params_with_defaults(&call_args, param_names, param_has_defaults)
        else {
            continue;
        };

        let mut instantiated = match instantiate_fun_sig_for_call(
            &callee_fqn,
            call_expr.span,
            cand,
            std::iter::once(GenericArgConstraint {
                expected: cand.params[0],
                found: actual_receiver_ty,
                found_is_placeholder: false,
                from: "接收者（receiver）".to_string(),
                span: receiver.span,
            })
            .chain(mapping.iter().copied().enumerate().filter_map(
                |(param_idx, arg_idx)| {
                    let Some(arg_idx) = arg_idx else {
                        return None;
                    };
                    let arg = &call_args[arg_idx];
                    Some(GenericArgConstraint {
                        expected: cand.params[param_idx + 1],
                        found: arg.ty,
                        found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                        from: format!("第 {} 个实参", arg_idx + 1),
                        span: arg.expr.span,
                    })
                },
            )),
            lower,
            builtins,
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // receiver mismatch 检查：同单候选路径，若 receiver 的期望类型依赖 `E`，
        // 必须等到 `E` 推断/实例化后才能确定 receiver 是否匹配（T0624）。
        let receiver_uses_eff = cand.eff_param.is_some()
            && cand
                .param_eff_row_var_subst
                .get(0)
                .is_some_and(|p| p.uses_eff_var());
        let mut cand_expected_receiver_ty = instantiated
            .params
            .first()
            .copied()
            .unwrap_or(cand.params[0]);
        if !receiver_uses_eff {
            if !is_type_assignable(
                actual_receiver_ty,
                cand_expected_receiver_ty,
                lower,
                builtins,
            ) {
                continue;
            }
        }

        // 只在需要时（lambda）进入 expected-context typecheck（与 direct call 多候选路径保持一致）。
        let mut ok = true;
        let mut checked_arg_tys: Vec<TypeId> = call_args.iter().map(|a| a.ty).collect();
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let arg = &call_args[arg_idx];
            if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                continue;
            }

            let expected_ty = instantiated.params[param_idx + 1];
            let found_ty = match infer_expr_type_in_expected_context(
                source,
                arg.expr,
                expected_ty,
                ExpectedTypeFrom::new(format!(
                    "`{}` 的第 {} 个形参 `{}`",
                    callee_fqn,
                    param_idx + 2,
                    cand.param_names[param_idx + 1]
                )),
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            ) {
                Ok(ty) => ty,
                Err(_) => {
                    ok = false;
                    break;
                }
            };
            checked_arg_tys[arg_idx] = found_ty;
        }
        if !ok {
            continue;
        }

        // T0509/T0624/T0628a：推断 `eff` row 参数：
        // - 从 lambda body 的 required effects 推断（`found - base`）；
        // - 从 `Type<eff Row>` receiver/形参的实参类型提取 row 约束（`found - base`）。
        let eff_arg = if let Some(eff_param) = &cand.eff_param {
            let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

            if let Some(base) = cand
                .param_nominal_eff_eff_base
                .get(0)
                .and_then(|b| b.as_ref())
            {
                let base = match substitute_type_args_in_effect_row(
                    base.clone(),
                    &cand.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                ) {
                    Ok(row) => row,
                    Err(_) => continue,
                };
                if let Some(found_row) = nominal_eff_row_from_type(actual_receiver_ty, lower) {
                    let delta = effect_row_difference(&found_row, &base);
                    terms.extend(delta.terms);
                }
            }

            for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                let Some(arg_idx) = arg_idx else {
                    continue;
                };
                let sig_param_idx = param_idx + 1; // 跳过 receiver

                if let Some(base) = cand
                    .param_nominal_eff_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                {
                    let base = match substitute_type_args_in_effect_row(
                        base.clone(),
                        &cand.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    ) {
                        Ok(row) => row,
                        Err(_) => continue,
                    };
                    let found_ty = checked_arg_tys[arg_idx];
                    if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                        let delta = effect_row_difference(&found_row, &base);
                        terms.extend(delta.terms);
                    }
                }

                let Some(base) = cand
                    .param_fn_effect_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                else {
                    continue;
                };
                let arg = &call_args[arg_idx];
                if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                    continue;
                }

                let base = match substitute_type_args_in_effect_row(
                    base.clone(),
                    &cand.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                ) {
                    Ok(row) => row,
                    Err(_) => continue,
                };
                let found_ty = checked_arg_tys[arg_idx];
                if let TypeKind::Ref(RefTypeKind::Function(found_fun)) = lower.type_kind(found_ty) {
                    let delta = effect_row_difference(&found_fun.effects, &base);
                    terms.extend(delta.terms);
                }
            }

            let inferred = EffectRow::new(terms);
            match substitute_type_args_in_effect_row(
                inferred,
                &cand.type_params,
                &instantiated.type_args,
                lower,
                call_expr.span,
            ) {
                Ok(row) => row,
                Err(_) => continue,
            }
        } else {
            EffectRow::pure()
        };

        if cand.eff_param.is_some()
            && instantiate_eff_row_var_in_sig_types(
                cand,
                &mut instantiated,
                &eff_arg,
                lower,
                call_expr.span,
            )
            .is_err()
        {
            ok = false;
        }
        if !ok {
            continue;
        }

        // 若 receiver 依赖 `E`，现在 `E` 已实例化完毕，补做 receiver mismatch 检查。
        if receiver_uses_eff {
            cand_expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(cand.params[0]);
            if !is_type_assignable(
                actual_receiver_ty,
                cand_expected_receiver_ty,
                lower,
                builtins,
            ) {
                continue;
            }
        }

        // 参数可赋值检查（跳过 receiver；只检查显式实参）。
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &call_args[arg_idx];
            let found_ty = checked_arg_tys[arg_idx];

            if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                continue;
            }
            if arg.is_int_lit && is_integer_type(expected_ty, lower, builtins) {
                continue;
            }
            ok = false;
            break;
        }

        if ok {
            let defaults_used = mapping.iter().filter(|x| x.is_none()).count();
            let mut expected_arg_tys = vec![builtins.nothing; call_args.len()];
            for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                let Some(arg_idx) = arg_idx else {
                    continue;
                };
                expected_arg_tys[arg_idx] = instantiated.params[param_idx + 1];
            }

            matched.push(MatchedExtensionOverload {
                sig: cand,
                receiver_ty: cand_expected_receiver_ty,
                expected_arg_tys,
                instantiated,
                eff_arg,
                defaults_used,
            });
        }
    }

    let chosen = match matched.len() {
        0 => {
            return Err(ExprTypeError::NoMatchingOverload {
                callee: callee_fqn,
                span: call_expr.span.into(),
            });
        }
        1 => matched.pop().expect("len == 1"),
        _ => {
            let Some(idx) = pick_most_specific_extension_overload(&matched, lower, builtins) else {
                let name = short_name_from_fqn(&callee_fqn).to_string();
                let candidates = join_overload_signatures(
                    matched
                        .iter()
                        .map(|c| {
                            fmt_overload_signature(
                                &name,
                                Some(c.receiver_ty),
                                c.instantiated.params.get(1..).unwrap_or_default(),
                                lower,
                            )
                        })
                        .collect(),
                );
                return Err(ExprTypeError::AmbiguousOverload {
                    callee: callee_fqn,
                    candidates,
                    span: call_expr.span.into(),
                });
            };
            matched.swap_remove(idx)
        }
    };

    check_unsafe_call_gate(&callee_fqn, chosen.sig, call_expr.span, lower)?;

    // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
    let type_param_bindings = type_param_bindings_from_sig(&chosen.sig.type_params, lower);
    lower.push_type_param_bindings(type_param_bindings);
    let eff_binding_pushed = if let Some(eff_param) = &chosen.sig.eff_param {
        lower.push_effect_row_param_binding(eff_param.name.clone(), chosen.eff_arg.clone());
        true
    } else {
        false
    };
    let lowered_effects = lower.lower_effect_row_expr(chosen.sig.effects.as_ref());
    if eff_binding_pushed {
        lower.pop_effect_row_param_binding();
    }
    lower.pop_type_param_bindings();
    let call_effects = substitute_type_args_in_effect_row(
        lowered_effects?,
        &chosen.sig.type_params,
        &chosen.instantiated.type_args,
        lower,
        call_expr.span,
    )?;
    for effect in call_effects.terms.iter().copied() {
        lower.record_performed_effect(effect, call_expr.span);
    }

    let ret = if safe {
        lower.ty_option(chosen.instantiated.return_ty)
    } else {
        chosen.instantiated.return_ty
    };

    Ok(ret)
}

/// 收集“当前文件内”的顶层 `val/var` 声明类型（FQN → TypeId）。
///
/// 说明：
/// - 顶层变量的类型注解由 `typecheck::check_file_headers` 强制要求，因此这里可以直接做 lowering；
/// - 该表用于处理表达式中的 `ResolvedValueRef::TopLevel`（变量引用）。
fn collect_top_level_value_types(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
) -> Result<HashMap<String, TypeId>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, TypeId> = HashMap::new();

    for item in &file.items {
        let ast::Item::Val(v) = item else {
            continue;
        };

        let ast::ValBinding::Name(name) = &v.binding else {
            // 顶层 pattern binding 会在 headers check 中报错；这里仅保持健壮性。
            continue;
        };

        let Some(ty_ref) = &v.ty else {
            continue;
        };

        let local_name = source.slice(name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };

        let ty = lower.lower_type_ref(ty_ref)?;
        map.insert(fqn, ty);
    }

    Ok(map)
}

/// 收集“当前文件内”的顶层 `fun` 声明签名（FQN → FunSig）。
///
/// 当前阶段（最小子集）：
/// - 支持 `fun <T>`：在签名 lowering 时把 `T` 视为 `TypeKind::Param`；
/// - 调用点的最小泛型实参推断见 T0505（当前仅支持单一类型参数）；
/// - 不处理 overload / default param；
/// - 未显式标注 return type 的函数，暂视为 `Unit`；
/// - 扩展函数会被降糖为“receiver 作为第一个参数”的普通顶层函数，用于 `receiver.member()` 与 `receiver?.member()`
///   调用的类型检查（spec §7.4）。
fn collect_top_level_fun_signatures(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<HashMap<String, Vec<FunSigOwned>>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, Vec<FunSigOwned>> = HashMap::new();

    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };

        let local_name = source.slice(fun.name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };
        let decl_span = fun.name.span;
        let builtin_flags = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations);

        // fun 自身的 type params 在签名 lowering 语境内可见。
        lower.push_type_params(&fun.type_params);

        // T0509：effect row 参数（`<eff E = Pure>`）。
        //
        // 说明：这里先把 `E` 绑定到默认值（缺省为 Pure），以便签名里的 `(...) / E` 能顺利 lowering；
        // 调用点会根据 lambda body 的 required effects 覆盖该默认值并做实例化。
        let eff_param_sig = if let Some(eff_param) = &fun.eff_param {
            let name = source.slice(eff_param.name.span).to_string();
            let default = match eff_param.default.as_ref() {
                Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
                None => EffectRow::pure(),
            };
            lower.push_effect_row_param_binding(name.clone(), default.clone());
            Some(EffParamSig { name, default })
        } else {
            None
        };

        let result: Result<(), ExprTypeError> = (|| {
            let type_params: Vec<TypeId> = fun
                .type_params
                .iter()
                .map(|p| lower.ty_param_from_decl(p))
                .collect();

            let mut param_names = Vec::with_capacity(fun.params.len() + 1);
            let mut param_has_defaults = Vec::with_capacity(fun.params.len() + 1);
            let mut params = Vec::with_capacity(fun.params.len() + 1);
            let mut param_fn_effect_eff_base: Vec<Option<EffectRow>> =
                Vec::with_capacity(fun.params.len() + 1);
            let mut param_nominal_eff_eff_base: Vec<Option<EffectRow>> =
                Vec::with_capacity(fun.params.len() + 1);
            let mut param_eff_row_var_subst: Vec<EffRowVarSubstPlan> =
                Vec::with_capacity(fun.params.len() + 1);

            // spec §7.4：扩展函数编译为普通静态函数：receiver 作为第一个参数。
            // typecheck 阶段也沿用这一“降糖”形式，便于统一调用检查逻辑。
            let is_extension = fun.receiver.is_some();
            let is_inline = fun.modifiers.contains(&ast::Modifier::Inline);
            if let Some(receiver) = &fun.receiver {
                // receiver 本身没有名字；这里用占位符保持与 `params` 对齐。
                param_names.push("<receiver>".to_string());
                param_has_defaults.push(false);
                let receiver_ty = lower.lower_type_ref(receiver)?;
                params.push(receiver_ty);
                param_fn_effect_eff_base.push(None);
                let nominal_eff_base = if let Some(eff_param) = &eff_param_sig {
                    type_ref_nominal_eff_eff_base(receiver, &eff_param.name, source, lower)?
                } else {
                    None
                };
                param_nominal_eff_eff_base.push(nominal_eff_base);
                let subst_plan = if let Some(eff_param) = &eff_param_sig {
                    build_eff_row_var_subst_plan(
                        receiver,
                        receiver_ty,
                        &eff_param.name,
                        source,
                        lower,
                    )?
                } else {
                    EffRowVarSubstPlan::None
                };
                param_eff_row_var_subst.push(subst_plan);
            }

            for p in &fun.params {
                let Some(ty_ref) = &p.ty else {
                    // headers check 已保证参数类型注解存在；这里保持健壮性。
                    continue;
                };
                let fn_eff_base = if let Some(eff_param) = &eff_param_sig {
                    type_ref_fn_effect_eff_base(ty_ref, &eff_param.name, source, lower)?
                } else {
                    None
                };
                let nominal_eff_base = if let Some(eff_param) = &eff_param_sig {
                    type_ref_nominal_eff_eff_base(ty_ref, &eff_param.name, source, lower)?
                } else {
                    None
                };
                param_names.push(source.slice(p.name.span).to_string());
                param_has_defaults.push(p.default_value.is_some());
                let ty = lower.lower_type_ref(ty_ref)?;
                params.push(ty);
                param_fn_effect_eff_base.push(fn_eff_base);
                param_nominal_eff_eff_base.push(nominal_eff_base);
                let subst_plan = if let Some(eff_param) = &eff_param_sig {
                    build_eff_row_var_subst_plan(ty_ref, ty, &eff_param.name, source, lower)?
                } else {
                    EffRowVarSubstPlan::None
                };
                param_eff_row_var_subst.push(subst_plan);
            }

            // T0623：`async fun foo(): T` 对外暴露 `Task<T>`。
            //
            // 说明：
            // - 这里的 `return_ty` 用于调用点类型与 overload resolution；
            // - 函数体内部的 `return` 类型检查仍以 AST 上的 `return_ty`（T）为准（见 `check_fun_body_exprs`）。
            let is_async_fun = fun.modifiers.contains(&ast::Modifier::Async);
            let inner_return_ty = match &fun.return_ty {
                Some(ret) => lower.lower_type_ref(ret)?,
                None => builtins.unit,
            };
            let return_ty = if is_async_fun {
                lower.lower_type_fqn_with_args(
                    TASK_FQN.to_string(),
                    vec![inner_return_ty],
                    fun.name.span,
                )?
            } else {
                inner_return_ty
            };

            let return_eff_row_var_subst = if let (Some(eff_param), Some(ret_ref)) =
                (eff_param_sig.as_ref(), fun.return_ty.as_ref())
            {
                if is_async_fun {
                    // 对 eff var substitution：在签名视图下，返回类型是 `Task<ret_ref>`。
                    let synth_span = ret_ref.span();
                    let synth_ret_ref = ast::TypeRef::Path(ast::TypePath {
                        span: synth_span,
                        segments: vec![
                            ast::Ident::synthetic(synth_span, "scoop"),
                            ast::Ident::synthetic(synth_span, "core"),
                            ast::Ident::synthetic(synth_span, "Task"),
                        ],
                        args: vec![ret_ref.clone()],
                    });
                    build_eff_row_var_subst_plan(
                        &synth_ret_ref,
                        return_ty,
                        &eff_param.name,
                        source,
                        lower,
                    )?
                } else {
                    build_eff_row_var_subst_plan(ret_ref, return_ty, &eff_param.name, source, lower)?
                }
            } else {
                EffRowVarSubstPlan::None
            };

            map.entry(fqn).or_default().push(FunSigOwned {
                decl_span,
                is_extension,
                is_inline,
                is_unsafe: builtin_flags.is_unsafe,
                is_nogc: builtin_flags.is_nogc,
                is_extern: builtin_flags.is_extern,
                is_intrinsic: builtin_flags.is_intrinsic,
                param_names,
                param_has_defaults,
                type_params,
                eff_param: eff_param_sig.clone(),
                param_fn_effect_eff_base,
                param_nominal_eff_eff_base,
                param_eff_row_var_subst,
                return_eff_row_var_subst,
                params,
                return_ty,
                effects: fun.effects.clone(),
            });
            Ok(())
        })();
        if eff_param_sig.is_some() {
            lower.pop_effect_row_param_binding();
        }
        lower.pop_type_params(&fun.type_params);
        result?;
    }

    Ok(map)
}

#[derive(Debug, Clone)]
struct InstantiatedFunSig {
    params: Vec<TypeId>,
    return_ty: TypeId,
    /// 推断/显式提供的泛型实参（与 `sig.type_params` 对齐）。
    ///
    /// 当前阶段（T0505）仅支持单一类型参数；未来可扩展为多参数。
    type_args: Vec<TypeId>,
}

#[derive(Debug, Clone)]
struct GenericArgConstraint {
    expected: TypeId,
    found: TypeId,
    /// 若为 `true`，表示 `found` 只是“为了 overload 筛选占位”的类型（例如 lambda 在预收集阶段被记为 `Any`），
    /// 不应当用于泛型推断。
    found_is_placeholder: bool,
    /// 该约束来自哪里（用于 diagnostics；例如“第 2 个实参”/“receiver”）。
    from: String,
    /// 约束来源对应的 span（用于把推断失败映射回具体位置）。
    span: Span,
}

fn effect_row_base_excluding_eff_var(
    row: &ast::EffectRowExpr,
    eff_name: &str,
    source: &SourceFile,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<EffectRow>, ExprTypeError> {
    if row.terms.is_empty() {
        return Ok(None);
    }

    let mut used = false;
    let mut base_terms: Vec<ast::TypePath> = Vec::with_capacity(row.terms.len());

    for term in &row.terms {
        let is_eff_var = term.segments.len() == 1
            && term.args.is_empty()
            && term.segments[0].text(source) == eff_name;
        if is_eff_var {
            used = true;
            continue;
        }
        base_terms.push(term.clone());
    }

    if !used {
        return Ok(None);
    }

    let base_expr = ast::EffectRowExpr {
        span: row.span,
        terms: base_terms,
        // `!` 的语义当前仅在函数声明处使用（见 T0626/T0627）。
        // 对函数类型/`Type<eff ...>` 的 row 这里只保留结构信息以便未来扩展。
        closed: row.closed,
    };

    Ok(Some(lower.lower_effect_row_expr(Some(&base_expr))?))
}

fn type_ref_fn_effect_eff_base(
    ty_ref: &ast::TypeRef,
    eff_name: &str,
    source: &SourceFile,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<EffectRow>, ExprTypeError> {
    let ast::TypeRef::Function(fun) = ty_ref else {
        return Ok(None);
    };

    let Some(effects) = fun.effects.as_ref() else {
        return Ok(None);
    };

    effect_row_base_excluding_eff_var(effects, eff_name, source, lower)
}

/// `Type<eff Row>`：use-site effect row 实参引用函数级 `eff` 变量（例如 `eff E` / `eff (E + IO)`）。
///
/// 返回值：
/// - `Ok(None)`：不引用 `E`
/// - `Ok(Some(base))`：引用了 `E`，其中 `base` 为把 `E` 移除后剩余的常量项（已 lowering）
fn type_ref_nominal_eff_eff_base(
    ty_ref: &ast::TypeRef,
    eff_name: &str,
    source: &SourceFile,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<EffectRow>, ExprTypeError> {
    match ty_ref {
        ast::TypeRef::Nullable { inner, .. } => {
            type_ref_nominal_eff_eff_base(inner, eff_name, source, lower)
        }
        ast::TypeRef::Path(path) => {
            let Some(ast::TypeRef::EffectRowArg { row, .. }) = path
                .args
                .iter()
                .find(|a| matches!(a, ast::TypeRef::EffectRowArg { .. }))
            else {
                return Ok(None);
            };

            effect_row_base_excluding_eff_var(row, eff_name, source, lower)
        }
        _ => Ok(None),
    }
}

fn type_param_name(ty: TypeId, lower: &TypeLowering<'_>) -> String {
    match lower.type_kind(ty) {
        TypeKind::Param(p) => p.name,
        _ => "<type param>".to_string(),
    }
}

fn collect_type_arg_candidates_for_single_type_param(
    expected: TypeId,
    found: TypeId,
    param_ty: TypeId,
    out: &mut Vec<TypeId>,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
    found_is_placeholder: bool,
) {
    if expected == param_ty {
        if found == builtins.nothing {
            return;
        }
        if found_is_placeholder && found == builtins.any {
            return;
        }
        out.push(found);
        return;
    }

    let expected_kind = lower.type_kind(expected);
    let found_kind = lower.type_kind(found);

    match (expected_kind, found_kind) {
        (
            TypeKind::Value(ValueTypeKind::Option(expected_inner)),
            TypeKind::Value(ValueTypeKind::Option(found_inner)),
        ) => {
            collect_type_arg_candidates_for_single_type_param(
                expected_inner,
                found_inner,
                param_ty,
                out,
                lower,
                builtins,
                found_is_placeholder,
            );
        }
        (
            TypeKind::Value(ValueTypeKind::Tuple(expected_elems)),
            TypeKind::Value(ValueTypeKind::Tuple(found_elems)),
        ) => {
            if expected_elems.len() != found_elems.len() {
                return;
            }
            for (e, f) in expected_elems.into_iter().zip(found_elems.into_iter()) {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
            TypeKind::Ref(RefTypeKind::Nominal(found_nominal)),
        ) => {
            if expected_nominal.fqn != found_nominal.fqn {
                return;
            }
            if expected_nominal.args.len() != found_nominal.args.len() {
                return;
            }
            for (e, f) in expected_nominal
                .args
                .into_iter()
                .zip(found_nominal.args.into_iter())
            {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }
        }
        (
            TypeKind::Value(ValueTypeKind::Nominal(expected_nominal)),
            TypeKind::Value(ValueTypeKind::Nominal(found_nominal)),
        ) => {
            if expected_nominal.fqn != found_nominal.fqn {
                return;
            }
            if expected_nominal.args.len() != found_nominal.args.len() {
                return;
            }
            for (e, f) in expected_nominal
                .args
                .into_iter()
                .zip(found_nominal.args.into_iter())
            {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Function(expected_fun)),
            TypeKind::Ref(RefTypeKind::Function(found_fun)),
        ) => {
            if expected_fun.receiver.is_some() != found_fun.receiver.is_some() {
                return;
            }
            if expected_fun.params.len() != found_fun.params.len() {
                return;
            }

            if let (Some(e), Some(f)) = (expected_fun.receiver, found_fun.receiver) {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }

            for (e, f) in expected_fun
                .params
                .into_iter()
                .zip(found_fun.params.into_iter())
            {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }

            collect_type_arg_candidates_for_single_type_param(
                expected_fun.return_ty,
                found_fun.return_ty,
                param_ty,
                out,
                lower,
                builtins,
                found_is_placeholder,
            );
        }
        _ => {}
    }
}

fn substitute_single_type_param(
    ty: TypeId,
    param_ty: TypeId,
    arg_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<TypeId, ExprTypeError> {
    if ty == param_ty {
        return Ok(arg_ty);
    }

    match lower.type_kind(ty) {
        TypeKind::Param(_) => Ok(ty),
        TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String) => Ok(ty),
        TypeKind::Value(ValueTypeKind::Unit)
        | TypeKind::Value(ValueTypeKind::Nothing)
        | TypeKind::Value(ValueTypeKind::Bool)
        | TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::IntN(_))
        | TypeKind::Value(ValueTypeKind::UIntN(_)) => Ok(ty),
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let new_inner = substitute_single_type_param(inner, param_ty, arg_ty, lower, use_span)?;
            if new_inner == inner {
                return Ok(ty);
            }
            Ok(lower.ty_option(new_inner))
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let mut changed = false;
            let mut out: Vec<TypeId> = Vec::with_capacity(elements.len());
            for e in elements {
                let new_e = substitute_single_type_param(e, param_ty, arg_ty, lower, use_span)?;
                if new_e != e {
                    changed = true;
                }
                out.push(new_e);
            }
            if !changed {
                return Ok(ty);
            }
            Ok(lower.ty_tuple(out))
        }
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            let mut changed = false;
            let mut args: Vec<TypeId> = Vec::with_capacity(nominal.args.len());
            for a in nominal.args {
                let new_a = substitute_single_type_param(a, param_ty, arg_ty, lower, use_span)?;
                if new_a != a {
                    changed = true;
                }
                args.push(new_a);
            }

            // T0624：名义类型的 `eff` row 参数同样需要参与 substitution（例如 `Raise<T>` 出现在 row 里）。
            let eff = match nominal.eff {
                Some(row) => {
                    let mut eff_changed = false;
                    let mut out_terms: Vec<TypeId> = Vec::with_capacity(row.terms.len());
                    for term in row.terms {
                        let new_term =
                            substitute_single_type_param(term, param_ty, arg_ty, lower, use_span)?;
                        if new_term != term {
                            eff_changed = true;
                        }
                        out_terms.push(new_term);
                    }
                    if eff_changed {
                        changed = true;
                        Some(EffectRow::new(out_terms))
                    } else {
                        Some(EffectRow { terms: out_terms })
                    }
                }
                None => None,
            };

            if !changed {
                return Ok(ty);
            }

            Ok(lower.intern_type_kind(TypeKind::Ref(RefTypeKind::Nominal(
                crate::ty::NominalType {
                    fqn: nominal.fqn,
                    args,
                    eff,
                },
            ))))
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let mut changed = false;
            let mut args: Vec<TypeId> = Vec::with_capacity(nominal.args.len());
            for a in nominal.args {
                let new_a = substitute_single_type_param(a, param_ty, arg_ty, lower, use_span)?;
                if new_a != a {
                    changed = true;
                }
                args.push(new_a);
            }

            let eff = match nominal.eff {
                Some(row) => {
                    let mut eff_changed = false;
                    let mut out_terms: Vec<TypeId> = Vec::with_capacity(row.terms.len());
                    for term in row.terms {
                        let new_term =
                            substitute_single_type_param(term, param_ty, arg_ty, lower, use_span)?;
                        if new_term != term {
                            eff_changed = true;
                        }
                        out_terms.push(new_term);
                    }
                    if eff_changed {
                        changed = true;
                        Some(EffectRow::new(out_terms))
                    } else {
                        Some(EffectRow { terms: out_terms })
                    }
                }
                None => None,
            };

            if !changed {
                return Ok(ty);
            }

            Ok(
                lower.intern_type_kind(TypeKind::Value(ValueTypeKind::Nominal(
                    crate::ty::NominalType {
                        fqn: nominal.fqn,
                        args,
                        eff,
                    },
                ))),
            )
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let mut changed = false;

            let receiver = match fun.receiver {
                Some(r) => {
                    let new_r = substitute_single_type_param(r, param_ty, arg_ty, lower, use_span)?;
                    if new_r != r {
                        changed = true;
                    }
                    Some(new_r)
                }
                None => None,
            };

            let mut params: Vec<TypeId> = Vec::with_capacity(fun.params.len());
            for p in fun.params {
                let new_p = substitute_single_type_param(p, param_ty, arg_ty, lower, use_span)?;
                if new_p != p {
                    changed = true;
                }
                params.push(new_p);
            }

            let return_ty =
                substitute_single_type_param(fun.return_ty, param_ty, arg_ty, lower, use_span)?;
            if return_ty != fun.return_ty {
                changed = true;
            }

            let mut effects_changed = false;
            let original_terms = fun.effects.terms;
            let mut effect_terms: Vec<TypeId> = Vec::with_capacity(original_terms.len());
            for e in original_terms {
                let new_e = substitute_single_type_param(e, param_ty, arg_ty, lower, use_span)?;
                if new_e != e {
                    effects_changed = true;
                }
                effect_terms.push(new_e);
            }
            let effects = if effects_changed {
                changed = true;
                EffectRow::new(effect_terms)
            } else {
                EffectRow {
                    terms: effect_terms,
                }
            };

            if !changed {
                return Ok(ty);
            }

            Ok(lower.ty_function(receiver, params, return_ty, effects))
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            let mut changed = false;
            let mut variants: Vec<TypeId> = Vec::with_capacity(union.variants.len());
            for v in union.variants {
                let new_v = substitute_single_type_param(v, param_ty, arg_ty, lower, use_span)?;
                if new_v != v {
                    changed = true;
                }
                variants.push(new_v);
            }
            if !changed {
                return Ok(ty);
            }
            Ok(lower.ty_union(variants))
        }
    }
}

/// 将签名类型里出现的 `E + base`（包含嵌套位置）统一实例化为 `E_arg + base`（T0628b）。
///
/// 说明：
/// - `sig` 来自“声明处默认 `E = default`”语境下的 lowering；
/// - `instantiated` 已完成 type args 的 substitution（T0505），但其内部仍可能残留：
///   - function type effects 上的默认 `E` 结果（例如默认 `Pure`）
///   - nominal use-site `eff` 实参里的默认 `E` 结果
/// - 该函数只负责把这些位置替换为调用点推断出的 `eff_arg`，并返回新的 `TypeId`。
fn instantiate_eff_row_var_in_sig_types(
    sig: &FunSigOwned,
    instantiated: &mut InstantiatedFunSig,
    eff_arg: &EffectRow,
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<(), ExprTypeError> {
    if sig.eff_param.is_none() {
        return Ok(());
    }

    if instantiated.params.len() != sig.param_eff_row_var_subst.len() {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "eff row substitution（sig/instantiated param arity mismatch）",
            span: use_span.into(),
        });
    }

    for (idx, plan) in sig.param_eff_row_var_subst.iter().enumerate() {
        if !plan.uses_eff_var() {
            continue;
        }
        let cur = instantiated.params[idx];
        instantiated.params[idx] = apply_eff_row_var_subst_plan(
            cur,
            plan,
            eff_arg,
            &sig.type_params,
            &instantiated.type_args,
            lower,
            use_span,
        )?;
    }

    if sig.return_eff_row_var_subst.uses_eff_var() {
        instantiated.return_ty = apply_eff_row_var_subst_plan(
            instantiated.return_ty,
            &sig.return_eff_row_var_subst,
            eff_arg,
            &sig.type_params,
            &instantiated.type_args,
            lower,
            use_span,
        )?;
    }

    Ok(())
}

/// `found - base`：用于从 `found ⊆ (E + base)` 这类约束中提取 `E` 的最小增量项。
fn effect_row_difference(found: &EffectRow, base: &EffectRow) -> EffectRow {
    if found.terms.is_empty() {
        return EffectRow::pure();
    }
    if base.terms.is_empty() {
        return found.clone();
    }

    // terms 已排序；线性差集即可。
    let mut out: Vec<TypeId> = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < found.terms.len() {
        if j >= base.terms.len() {
            out.extend(found.terms[i..].iter().copied());
            break;
        }

        let a = found.terms[i];
        let b = base.terms[j];
        if a == b {
            i += 1;
            j += 1;
            continue;
        }
        if a < b {
            out.push(a);
            i += 1;
            continue;
        }
        // a > b：base 继续前进尝试追上 a
        j += 1;
    }

    EffectRow::new(out)
}

fn nominal_eff_row_from_type(ty: TypeId, lower: &TypeLowering<'_>) -> Option<EffectRow> {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => nominal.eff,
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.eff,
        // nullable（`T?`）在 lowering 阶段会变成 `Option<T>`；这里递归剥一层便于推断 `E`。
        TypeKind::Value(ValueTypeKind::Option(inner)) => nominal_eff_row_from_type(inner, lower),
        _ => None,
    }
}

fn type_param_bindings_from_sig(
    type_params: &[TypeId],
    lower: &TypeLowering<'_>,
) -> Vec<(String, TypeId)> {
    type_params
        .iter()
        .copied()
        .filter_map(|ty| match lower.type_kind(ty) {
            TypeKind::Param(p) => Some((p.name, ty)),
            _ => None,
        })
        .collect()
}

fn substitute_type_args_in_effect_row(
    row: EffectRow,
    type_params: &[TypeId],
    type_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<EffectRow, ExprTypeError> {
    if type_params.is_empty() || type_args.is_empty() {
        return Ok(row);
    }

    let mut out_terms: Vec<TypeId> = Vec::with_capacity(row.terms.len());
    for effect in row.terms {
        let mut cur = effect;
        for (param_ty, arg_ty) in type_params.iter().copied().zip(type_args.iter().copied()) {
            cur = substitute_single_type_param(cur, param_ty, arg_ty, lower, use_span)?;
        }
        out_terms.push(cur);
    }

    Ok(EffectRow::new(out_terms))
}

fn instantiate_fun_sig_for_call(
    callee: &str,
    call_span: Span,
    sig: &FunSigOwned,
    constraints: impl IntoIterator<Item = GenericArgConstraint>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<InstantiatedFunSig, ExprTypeError> {
    if sig.type_params.is_empty() {
        return Ok(InstantiatedFunSig {
            params: sig.params.clone(),
            return_ty: sig.return_ty,
            type_args: Vec::new(),
        });
    }

    // T0505 v0：先只支持单一类型参数。
    if sig.type_params.len() != 1 {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "generic call（multiple type params）",
            span: call_span.into(),
        });
    }

    let param_ty = sig.type_params[0];
    let param_name = type_param_name(param_ty, lower);

    #[derive(Debug, Clone)]
    struct InferredTypeArgSource {
        from: String,
        span: Span,
    }

    // T0510：推断失败诊断（最小可读解释）：
    // - 当前阶段（T0505）仅支持单一类型参数，因此这里用“逐候选 unify + 记录来源”的方式，
    //   以便在冲突时把推断失败精确映射到“产生冲突的那一条约束”的 span 上。
    let mut inferred: Option<(TypeId, InferredTypeArgSource)> = None;
    for c in constraints {
        let mut candidates: Vec<TypeId> = Vec::new();
        collect_type_arg_candidates_for_single_type_param(
            c.expected,
            c.found,
            param_ty,
            &mut candidates,
            lower,
            builtins,
            c.found_is_placeholder,
        );

        for candidate in candidates {
            match &mut inferred {
                None => {
                    inferred = Some((
                        candidate,
                        InferredTypeArgSource {
                            from: c.from.clone(),
                            span: c.span,
                        },
                    ));
                }
                Some((bound, _)) if *bound == candidate => {}
                Some((bound, src)) => {
                    return Err(ExprTypeError::GenericTypeArgInferenceConflict {
                        callee: callee.to_string(),
                        param: param_name,
                        left: lower.fmt_type(*bound),
                        right: lower.fmt_type(candidate),
                        left_from: src.from.clone(),
                        right_from: c.from.clone(),
                        span: c.span.into(),
                        previous: src.span.into(),
                    });
                }
            }
        }
    }

    let Some((binding, _)) = inferred else {
        return Err(ExprTypeError::GenericTypeArgNotInferred {
            callee: callee.to_string(),
            param: param_name,
            span: call_span.into(),
        });
    };

    let mut params: Vec<TypeId> = Vec::with_capacity(sig.params.len());
    for p in sig.params.iter().copied() {
        params.push(substitute_single_type_param(
            p, param_ty, binding, lower, call_span,
        )?);
    }
    let return_ty =
        substitute_single_type_param(sig.return_ty, param_ty, binding, lower, call_span)?;

    Ok(InstantiatedFunSig {
        params,
        return_ty,
        type_args: vec![binding],
    })
}

#[derive(Debug, Clone, Copy)]
struct CallTargetSig<'a> {
    sig: &'a FunSigOwned,
    /// `args[i]` 对应到 `sig.params[i + arg_param_offset]`。
    arg_param_offset: usize,
}

fn is_function_type(ty: TypeId, lower: &TypeLowering<'_>) -> bool {
    matches!(lower.type_kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
}

fn resolve_call_target_for_expr_stmt<'a>(
    source: &SourceFile,
    callee: &ast::Expr,
    lower: &TypeLowering<'_>,
    top_level_funs: &'a HashMap<String, Vec<FunSigOwned>>,
) -> Option<CallTargetSig<'a>> {
    match &callee.kind {
        ast::ExprKind::Ident(id) => {
            let resolved = id.resolved.as_ref()?;
            let ast::ResolvedValueRef::TopLevel { fqn } = resolved else {
                return None;
            };

            let sigs = top_level_funs.get(fqn)?;

            // 扩展函数不能以 `f(args...)` 的形式被直接调用：这里只考虑普通顶层函数候选。
            let mut direct_call_candidates = sigs.iter().filter(|s| !s.is_extension);
            let sig = direct_call_candidates.next()?;
            if direct_call_candidates.next().is_some() {
                return None;
            }

            Some(CallTargetSig {
                sig,
                arg_param_offset: 0,
            })
        }
        ast::ExprKind::MemberAccess { member, .. }
        | ast::ExprKind::SafeMemberAccess { member, .. } => {
            let callee_fqn = match member.resolved.as_ref() {
                Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) => fqn.clone(),
                Some(ast::ResolvedMemberRef::Fun { .. })
                | Some(ast::ResolvedMemberRef::Value { .. })
                | Some(ast::ResolvedMemberRef::ExtensionValue { .. }) => return None,
                None => {
                    let name = source.slice(member.span);
                    if lower.pkg_prefix().is_empty() {
                        name.to_string()
                    } else {
                        format!("{}.{}", lower.pkg_prefix(), name)
                    }
                }
            };

            let sigs = top_level_funs.get(&callee_fqn)?;
            let mut ext_candidates = sigs.iter().filter(|s| s.is_extension);
            let sig = ext_candidates.next()?;
            if ext_candidates.next().is_some() {
                return None;
            }

            Some(CallTargetSig {
                sig,
                // 扩展调用：`receiver.member(args...)` 的第一个参数是 receiver。
                arg_param_offset: 1,
            })
        }
        _ => None,
    }
}

fn check_lambda_expr_stmt_body(
    source: &SourceFile,
    lam: &ast::LambdaExpr,
    allow_non_local_return: bool,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
    mutable_bindings: &HashSet<Span>,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 说明：当前阶段 lambda 仍未完整 typecheck；这里仅复用现有的“语句层递归”逻辑来：
    // - 捕获非法 `return`（non-local return 门禁，T0444）
    // - 避免 lambda 内的局部声明污染外层作用域（clone 快照）
    let mut lambda_locals = locals.clone();
    let mut lambda_stable = stable_bindings.clone();
    let mut lambda_mutable = mutable_bindings.clone();
    let nested_expected_return_ty = if allow_non_local_return {
        expected_return_ty
    } else {
        None
    };

    // required effects（T0604）：lambda body 的 effect 属于该函数值，不计入外层函数立即执行的 effects。
    lower.with_effect_collection_suspended(|lower| {
        check_expr_stmt(
            source,
            lam.body.as_ref(),
            lower,
            builtins,
            &mut lambda_locals,
            &mut lambda_stable,
            &mut lambda_mutable,
            0,
            nested_expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        )
    })
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

fn short_name_from_fqn(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

fn fmt_overload_signature(
    name: &str,
    receiver_ty: Option<TypeId>,
    params: &[TypeId],
    lower: &TypeLowering<'_>,
) -> String {
    let params = params
        .iter()
        .copied()
        .map(|ty| lower.fmt_type(ty))
        .collect::<Vec<_>>()
        .join(", ");

    match receiver_ty {
        Some(recv) => format!("{}.{}({})", lower.fmt_type(recv), name, params),
        None => format!("{name}({params})"),
    }
}

fn join_overload_signatures(mut sigs: Vec<String>) -> String {
    sigs.sort();
    sigs.dedup();
    sigs.join(" | ")
}

fn visibility_from_modifiers(modifiers: &[ast::Modifier]) -> Visibility {
    // 当前阶段（T0245）parser 只负责“解析并存储”，不做组合合法性校验；
    // 这里沿用 resolver 的最小优先级规则：`private` > `internal` > 默认 `public`。
    if modifiers.contains(&ast::Modifier::Private) {
        return Visibility::Private;
    }
    if modifiers.contains(&ast::Modifier::Internal) {
        return Visibility::Internal;
    }
    Visibility::Public
}

fn check_required_effects_for_fun_decl(
    fun: &ast::FunDecl,
    performed: &[(TypeId, Span)],
    is_entry_point: bool,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    // spec §5.10：entry point 由 runtime 在无 ambient handler 的边界调用，
    // 因此其 effect row 必须是 `Pure`（不能显式声明 non-Pure，也不能通过 internal/private 推断出效果）。
    if is_entry_point {
        if let Some(expr) = fun.effects.as_ref() {
            let row = lower.lower_effect_row_expr(Some(expr))?;
            if !row.terms.is_empty() {
                return Err(ExprTypeError::EntryPointMustBePure {
                    declared: fmt_effect_row(&row, lower),
                    span: expr.span.into(),
                });
            }
            // spec §5.8.4：entry point 属于 system boundary，必须是闭合 effect row（`Pure!`）。
            // 说明：省略 effects 标注时仍会按 entry point 的规则强制 Pure；这里仅对“显式写了 open row `/ Pure`”
            // 给出更明确的诊断，避免用户误以为 open row 能封住 callback/transitive effects。
            if !expr.closed {
                return Err(ExprTypeError::EntryPointMustBeClosedPure {
                    declared: fmt_effect_row(&row, lower),
                    span: expr.span.into(),
                });
            }
        }
    }

    // 即使函数体没有 perform（`performed.is_empty()`），也需要对“显式写出的 effects row”做最小语义校验：
    // - effect row item 必须是 effect 类型
    // - 闭合 row 不能直接引用 row 变量（例如 `E!`，T0628b）
    if !is_entry_point {
        if let Some(expr) = fun.effects.as_ref() {
            let _ = lower.lower_effect_row_expr(Some(expr))?;
        }
    }

    if performed.is_empty() {
        return Ok(());
    }

    // T0508：effect row 推断入口：
    // - entry point：强制为 Pure（spec §5.10）。
    // - public：缺省效果强制为 Pure（perform 任何 effect 都必须显式标注 row 或被 handler 捕获）
    // - private/internal：允许省略 `/ RowExpr`，由函数体内 “立即执行的 perform” 推断出 required effects。
    let declared = if is_entry_point {
        EffectRow::pure()
    } else if fun.effects.is_some() {
        lower.lower_effect_row_expr(fun.effects.as_ref())?
    } else {
        match visibility_from_modifiers(&fun.modifiers) {
            Visibility::Public => EffectRow::pure(),
            Visibility::Internal | Visibility::Private => {
                let mut seen: HashSet<TypeId> = HashSet::new();
                let mut terms: Vec<TypeId> = Vec::new();
                for (effect, _) in performed.iter().copied() {
                    if seen.insert(effect) {
                        terms.push(effect);
                    }
                }
                EffectRow::new(terms)
            }
        }
    };

    for (effect, span) in performed.iter().copied() {
        if declared.terms.contains(&effect) {
            continue;
        }

        return Err(ExprTypeError::RequiredEffectNotDeclared {
            required: lower.fmt_type(effect),
            declared: fmt_effect_row(&declared, lower),
            span: span.into(),
        });
    }

    Ok(())
}

fn check_fun_body_exprs(
    source: &SourceFile,
    fun_fqn: &str,
    fun: &ast::FunDecl,
    is_entry_point: bool,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &mut HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    lower.push_type_params(&fun.type_params);
    let eff_binding_pushed = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => match lower.lower_effect_row_expr(Some(expr)) {
                Ok(row) => row,
                Err(e) => {
                    lower.pop_type_params(&fun.type_params);
                    return Err(e.into());
                }
            },
            None => EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    let unsafe_ctx_pushed = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations).is_unsafe;
    if unsafe_ctx_pushed {
        lower.push_unsafe_context();
    }

    lower.begin_effect_collection();
    let body_result: Result<(), ExprTypeError> = (|| {
        // 函数级的“局部值类型表”（binder decl span → TypeId）。
        //
        // 当前阶段规则（最小子集）：
        // - 参数：必须有类型注解（由 headers check 保证），因此可直接 lowering；
        // - 局部 `val/var`：
        //   - 若显式写了 `: Type`，则以该类型为准，并校验 initializer（若存在）类型匹配；
        //   - 否则若有 initializer，则以 initializer 类型推导；
        //   - 都没有则当前阶段无法推导（后续任务再补齐规则）。
        let mut locals: HashMap<Span, TypeId> = HashMap::new();
        // 可用于 smart cast 的“稳定绑定”（当前阶段仅覆盖：参数 + `val`）。
        let mut stable_bindings: HashSet<Span> = HashSet::new();
        // 可赋值（mutable）的绑定：当前阶段仅覆盖局部 `var`。
        let mut mutable_bindings: HashSet<Span> = HashSet::new();

        // 扩展函数：为 `this` 注入隐式绑定（resolver 将 `this` 解析到 receiver 的 decl_span）。
        if let Some(receiver) = &fun.receiver {
            let receiver_ty = lower.lower_type_ref(receiver)?;
            locals.insert(receiver.span(), receiver_ty);
            stable_bindings.insert(receiver.span());
        }

        for p in &fun.params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            let ty = lower.lower_type_ref(ty_ref)?;
            locals.insert(p.name.span, ty);
            stable_bindings.insert(p.name.span);
        }

        // 该函数的期望返回类型（T0417）：用于 `return expr?` 的类型检查。
        let expected_return_ty = match &fun.return_ty {
            Some(ret) => lower.lower_type_ref(ret)?,
            None => match &fun.body {
                ast::FunBody::Block(b) => {
                    let inferred = try_infer_fun_return_ty_from_block(
                        source,
                        b,
                        lower,
                        builtins,
                        &mut locals,
                        &mut stable_bindings,
                        &mut mutable_bindings,
                        0,
                        top_level_types,
                        &*top_level_funs,
                        member_mutabilities,
                        struct_field_types,
                    )?
                    .unwrap_or(builtins.unit);

                    // 回写到顶层函数签名表：使得后续同文件的调用点能看到推断后的返回类型。
                    if let Some(sigs) = top_level_funs.get_mut(fun_fqn) {
                        if let Some(sig) = sigs.iter_mut().find(|s| s.decl_span == fun.name.span) {
                            sig.return_ty = if fun.modifiers.contains(&ast::Modifier::Async) {
                                lower.lower_type_fqn_with_args(
                                    TASK_FQN.to_string(),
                                    vec![inferred],
                                    fun.name.span,
                                )?
                            } else {
                                inferred
                            };
                        }
                    }

                    inferred
                }
                ast::FunBody::Missing => builtins.unit,
            },
        };

        match &fun.body {
            ast::FunBody::Block(b) => check_block_exprs(
                source,
                b,
                lower,
                builtins,
                &mut locals,
                &mut stable_bindings,
                &mut mutable_bindings,
                0,
                Some(expected_return_ty),
                top_level_types,
                &*top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?,
            ast::FunBody::Missing => {}
        }

        Ok(())
    })();
    let performed_effects = lower.finish_effect_collection();

    let result = match body_result {
        Ok(()) => {
            // T0623：`async fun` 的 `/ Async` 只存在于 Task 的计算上下文，
            // 因此函数体内的 `Async` performed effects 不应向外层（调用点）传播。
            let performed_for_decl = if fun.modifiers.contains(&ast::Modifier::Async) {
                let async_effect = lower.lower_type_fqn_with_args(
                    ASYNC_EFFECT_FQN.to_string(),
                    Vec::new(),
                    fun.name.span,
                )?;
                performed_effects
                    .iter()
                    .copied()
                    .filter(|(effect, _)| *effect != async_effect)
                    .collect::<Vec<_>>()
            } else {
                performed_effects.clone()
            };

            check_required_effects_for_fun_decl(fun, &performed_for_decl, is_entry_point, lower)?;
            Ok(())
        }
        Err(e) => Err(e),
    };
    if eff_binding_pushed {
        lower.pop_effect_row_param_binding();
    }
    if unsafe_ctx_pushed {
        lower.pop_unsafe_context();
    }
    lower.pop_type_params(&fun.type_params);
    result
}

fn check_block_exprs(
    source: &SourceFile,
    block: &ast::Block,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    loop_depth: usize,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 与 resolver 的作用域规则对齐：block 内声明仅在该 block 内可见。
    // 这里用“进入时快照 + 退出时回滚”的方式实现最小作用域，不引入额外的数据结构。
    let saved_locals = locals.clone();
    let saved_stable = stable_bindings.clone();
    let saved_mutable = mutable_bindings.clone();

    for stmt in &block.stmts {
        check_stmt_exprs(
            source,
            stmt,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        )?;
    }

    *locals = saved_locals;
    *stable_bindings = saved_stable;
    *mutable_bindings = saved_mutable;

    Ok(())
}

fn check_stmt_exprs(
    source: &SourceFile,
    stmt: &ast::Stmt,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    loop_depth: usize,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    match &stmt.kind {
        ast::StmtKind::Val(v) => check_local_val_decl_exprs(
            source,
            v,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?,
        ast::StmtKind::Expr(e) => check_expr_stmt(
            source,
            e,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        )?,
        ast::StmtKind::Return { return_span, value } => {
            let Some(expected) = expected_return_ty else {
                return Err(ExprTypeError::ReturnNotInFunctionBody {
                    span: (*return_span).into(),
                });
            };

            match value {
                Some(v) => {
                    let found = infer_expr_type_in_expected_context(
                        source,
                        v,
                        expected,
                        ExpectedTypeFrom::new("函数返回类型"),
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?;
                    if !is_type_assignable(found, expected, lower, builtins) {
                        return Err(ExprTypeError::ReturnTypeMismatch {
                            expected: lower.fmt_type(expected),
                            found: lower.fmt_type(found),
                            span: v.span.into(),
                        });
                    }
                }
                None => {
                    // `return` 不带返回值：等价于返回 `Unit`。
                    if expected != builtins.unit {
                        return Err(ExprTypeError::ReturnValueRequired {
                            expected: lower.fmt_type(expected),
                            span: (*return_span).into(),
                        });
                    }
                }
            }
        }
        ast::StmtKind::While { cond, body, .. } => {
            let cond_ty = infer_expr_type(
                source,
                cond,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;

            if !is_type_assignable(cond_ty, builtins.bool_, lower, builtins) {
                return Err(ExprTypeError::WhileConditionNotBool {
                    found: lower.fmt_type(cond_ty),
                    span: cond.span.into(),
                });
            }

            check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth + 1,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
        }
        ast::StmtKind::Break { break_span } => {
            if loop_depth == 0 {
                return Err(ExprTypeError::BreakNotInLoop {
                    span: (*break_span).into(),
                });
            }
        }
        ast::StmtKind::Continue { continue_span } => {
            if loop_depth == 0 {
                return Err(ExprTypeError::ContinueNotInLoop {
                    span: (*continue_span).into(),
                });
            }
        }
        ast::StmtKind::ComptimeBlock { body, .. } => {
            check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
        }
        ast::StmtKind::ComptimeIf(ci) => {
            check_block_exprs(
                source,
                &ci.then_branch,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
            if let Some(else_branch) = &ci.else_branch {
                match &**else_branch {
                    ast::ComptimeIfElse::Block(b) => check_block_exprs(
                        source,
                        b,
                        lower,
                        builtins,
                        locals,
                        stable_bindings,
                        mutable_bindings,
                        loop_depth,
                        expected_return_ty,
                        top_level_types,
                        top_level_funs,
                        member_mutabilities,
                        struct_field_types,
                    )?,
                    ast::ComptimeIfElse::If(next) => {
                        // 递归跟进 else-if 链。
                        let mut cur: &ast::ComptimeIf = next;
                        loop {
                            check_block_exprs(
                                source,
                                &cur.then_branch,
                                lower,
                                builtins,
                                locals,
                                stable_bindings,
                                mutable_bindings,
                                loop_depth,
                                expected_return_ty,
                                top_level_types,
                                top_level_funs,
                                member_mutabilities,
                                struct_field_types,
                            )?;
                            match &cur.else_branch {
                                Some(e) => match &**e {
                                    ast::ComptimeIfElse::Block(b) => {
                                        check_block_exprs(
                                            source,
                                            b,
                                            lower,
                                            builtins,
                                            locals,
                                            stable_bindings,
                                            mutable_bindings,
                                            loop_depth,
                                            expected_return_ty,
                                            top_level_types,
                                            top_level_funs,
                                            member_mutabilities,
                                            struct_field_types,
                                        )?;
                                        break;
                                    }
                                    ast::ComptimeIfElse::If(next) => cur = next,
                                },
                                None => break,
                            }
                        }
                    }
                }
            }
        }
        ast::StmtKind::ComptimeFor(cf) => {
            check_block_exprs(
                source,
                &cf.body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth + 1,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
        }
        ast::StmtKind::Empty | ast::StmtKind::Missing => {}
    }

    Ok(())
}

fn check_local_val_decl_exprs(
    source: &SourceFile,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let declared_ty = match &v.ty {
        Some(ty_ref) => Some(lower.lower_type_ref(ty_ref)?),
        None => None,
    };
    let expected_from = match &v.binding {
        ast::ValBinding::Name(name) => {
            ExpectedTypeFrom::new(format!("局部绑定 `{}` 的类型注解", source.slice(name.span)))
        }
        ast::ValBinding::Pattern(_) => ExpectedTypeFrom::new("局部解构绑定的类型注解"),
    };

    // 先类型检查 initializer（语义：局部变量在其声明之后可见，因此 init 内不能引用自身）。
    let init_ty = match &v.init {
        Some(init) => Some(match declared_ty {
            Some(expected) => infer_expr_type_in_expected_context(
                source,
                init,
                expected,
                expected_from.clone(),
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?,
            None => infer_expr_type(
                source,
                init,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?,
        }),
        None => None,
    };

    if let (Some(expected), Some(found)) = (declared_ty, init_ty) {
        if !is_type_assignable(found, expected, lower, builtins) {
            // 与顶层 initializer 一致：允许整数字面量被上下文整数类型吸收（后续可加入 range check）。
            if matches!(v.init.as_ref().unwrap().kind, ast::ExprKind::IntLit)
                && is_integer_type(expected, lower, builtins)
            {
                // ok
            } else {
                // 复用顶层 initializer 的错误码与文本（保持 fixtures 断言稳定）。
                let init = v.init.as_ref().unwrap();
                return Err(ExprTypeError::InitializerTypeMismatch {
                    expected: lower.fmt_type(expected),
                    found: lower.fmt_type(found),
                    span: init.span.into(),
                });
            }
        }
    }

    let inferred = declared_ty.or(init_ty);

    match &v.binding {
        ast::ValBinding::Name(name) => {
            let Some(ty) = inferred else {
                // 当前阶段不支持“无类型注解 + 无 initializer”的局部绑定推导。
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "局部 val/var（缺少类型与 initializer）",
                    span: name.span.into(),
                });
            };
            locals.insert(name.span, ty);
            match v.kind {
                ast::ValKind::Val => {
                    stable_bindings.insert(name.span);
                }
                ast::ValKind::Var => {
                    mutable_bindings.insert(name.span);
                }
            }
        }
        ast::ValBinding::Pattern(pat) => {
            // spec §4.2：`var` 不支持 destructuring patterns（只允许简单绑定）。
            if matches!(v.kind, ast::ValKind::Var) {
                return Err(ExprTypeError::DestructuringVarNotAllowed {
                    span: pat.span.into(),
                });
            }

            let Some(init_ty) = init_ty else {
                // parser 已强制 pattern binding 必须有 initializer；这里仅做健壮性兜底。
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "解构绑定（缺少 initializer）",
                    span: pat.span.into(),
                });
            };

            let bindings = val_pat::infer_val_pat_bindings(
                source,
                pat,
                init_ty,
                lower,
                builtins,
                struct_field_types,
            )?;

            // `val` 解构引入的绑定与普通 `val x = ...` 一样：
            // - 在其声明之后可见（resolver 已建立作用域）
            // - 属于稳定绑定，可用于 smart cast（当前阶段仅记录）
            for (decl_span, ty) in bindings {
                locals.insert(decl_span, ty);
                stable_bindings.insert(decl_span);
            }
        }
    }

    Ok(())
}

fn check_expr_stmt(
    source: &SourceFile,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    loop_depth: usize,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 当前阶段的表达式语句仅用于支持控制流结构内部的“局部 val/var 推导”回归：
    // - `if (...) { val ... } else { ... }`
    // - `call { ... }`：递归进入 lambda body 捕获非法 `return`（T0444）
    //
    // 其他表达式语句（例如单独的调用）暂不强制 typecheck，以避免在未实现更多 ExprKind
    // 的阶段引入大量不相关的回归失败。
    match &expr.kind {
        ast::ExprKind::Block(b) => check_block_exprs(
            source,
            b,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        ),
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => check_if_expr_stmt(
            source,
            cond.as_ref(),
            then_branch.as_ref(),
            else_branch.as_deref(),
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        ),
        ast::ExprKind::When { subject, arms } => {
            // `when` 表达式作为语句时：
            // - 递归进入分支 body，以覆盖其中的局部绑定/控制流；
            // - T0427：为每个 arm 建立独立的“局部类型表”快照，并注入 pattern binder 的类型。
            check_expr_stmt(
                source,
                subject.as_ref(),
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;

            let subject_ty = infer_expr_type(
                source,
                subject.as_ref(),
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )
            .ok();

            if let Some(subject_ty) = subject_ty {
                when_exhaustiveness::check_when_exhaustiveness(
                    source, expr, subject_ty, arms, lower, builtins,
                )?;
            }

            for arm in arms {
                let mut arm_locals = locals.clone();
                let mut arm_stable = stable_bindings.clone();
                let mut arm_mutable = mutable_bindings.clone();

                if let Some(subject_ty) = subject_ty {
                    for (decl_span, ty) in when_pat::infer_when_pat_bindings(
                        source, &arm.pat, subject_ty, lower, builtins,
                    )? {
                        arm_locals.insert(decl_span, ty);
                        arm_stable.insert(decl_span);
                    }
                }

                check_expr_stmt(
                    source,
                    &arm.body,
                    lower,
                    builtins,
                    &mut arm_locals,
                    &mut arm_stable,
                    &mut arm_mutable,
                    loop_depth,
                    expected_return_ty,
                    top_level_types,
                    top_level_funs,
                    member_mutabilities,
                    struct_field_types,
                )?;
            }
            Ok(())
        }
        ast::ExprKind::Handle {
            body,
            arms,
            finally,
        } => {
            // `handle` 在表达式语句位置仍需递归 typecheck：
            // - 以便捕获 handler arms 内的类型错误
            // - 以便正确记录 required effects（body 内被 handler 捕获的 effects 不应向外传播）
            let _ = infer_handle_expr_type(
                source,
                expr,
                body,
                arms,
                finally.as_ref(),
                lower,
                builtins,
                &*locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            Ok(())
        }
        ast::ExprKind::NotNullAssert {
            expr: inner,
            op_span,
        } => {
            // `!!` 的语义属于“立即执行的表达式”（会在运行期做 null assertion），
            // 因此即使它出现在表达式语句位置，也必须参与 typecheck/required-effects 收集（T0421b）。
            let _ = infer_not_null_assert_expr_type(
                source,
                inner.as_ref(),
                *op_span,
                lower,
                builtins,
                &*locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            Ok(())
        }
        ast::ExprKind::Cast { .. } => {
            // T0445：`x as T` 的失败语义会触发 `Raise<RuntimeError>`。
            // 与 `!!` 一样，它属于“立即执行的表达式”，即使出现在表达式语句位置也必须参与
            // required-effects 收集；否则 `/ Pure` 函数体内的 `as` 会被错误放过。
            match infer_expr_type(
                source,
                expr,
                lower,
                builtins,
                &*locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            ) {
                Ok(_) => Ok(()),
                Err(ExprTypeError::UnsupportedExpr { .. }) => Ok(()),
                Err(e) => Err(e),
            }
        }
        ast::ExprKind::Call { callee, args } => {
            // T0444：`inline` 与 non-local return 的最小语义门禁：
            // - 默认：lambda body 内出现 `return` 一律报错
            // - 例外：当该 lambda 是 inline 函数的“lambda 参数实参”时，允许 non-local return
            //
            // 注意：当前阶段不做完整的调用类型检查（包括 lambda 类型推导），这里只做结构化递归与门禁，
            // 以便在不引入更多 type inference 复杂度的前提下先把语义边界钉死。
            let target =
                resolve_call_target_for_expr_stmt(source, callee.as_ref(), lower, top_level_funs);

            // 递归进入 callee 与 args：保证 `f({ return ... })` 这类结构也能被覆盖。
            check_expr_stmt(
                source,
                callee.as_ref(),
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;

            for (idx, arg) in args.iter().enumerate() {
                let ast::ExprKind::Lambda(lam) = &arg.kind else {
                    check_expr_stmt(
                        source,
                        arg,
                        lower,
                        builtins,
                        locals,
                        stable_bindings,
                        mutable_bindings,
                        loop_depth,
                        expected_return_ty,
                        top_level_types,
                        top_level_funs,
                        member_mutabilities,
                        struct_field_types,
                    )?;
                    continue;
                };

                let allow_non_local_return = match target {
                    Some(t) if t.sig.is_inline => {
                        let param_idx = idx + t.arg_param_offset;
                        match t.sig.params.get(param_idx).copied() {
                            Some(ty) => is_function_type(ty, lower),
                            None => false,
                        }
                    }
                    _ => false,
                };

                check_lambda_expr_stmt_body(
                    source,
                    lam,
                    allow_non_local_return,
                    lower,
                    builtins,
                    locals,
                    stable_bindings,
                    mutable_bindings,
                    expected_return_ty,
                    top_level_types,
                    top_level_funs,
                    member_mutabilities,
                    struct_field_types,
                )?;
            }

            // required effects（T0604）：
            // call 作为“表达式语句”时，typecheck 默认不会对其做完整调用检查；
            // 但 effect op call（例如 `Raise.raise(e)`）属于“立即执行的 perform”，必须被记录。
            if let ast::ExprKind::MemberAccess { member, .. } = &callee.kind {
                let _ = infer_effect_op_call_expr_type(
                    source,
                    expr,
                    member,
                    args,
                    lower,
                    builtins,
                    &*locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
            }

            Ok(())
        }
        ast::ExprKind::Lambda(lam) => {
            // spec §7.3：默认不允许 lambda non-local return。
            //
            // 例外：当 lambda 作为 inline 函数调用的 lambda 实参时允许（见 `ExprKind::Call` 分支）。
            check_lambda_expr_stmt_body(
                source,
                lam,
                false,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )
        }
        ast::ExprKind::Assign { lhs, rhs, .. } => check_assign_expr_stmt(
            source,
            lhs,
            rhs,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        ),
        _ => Ok(()),
    }
}

fn check_if_expr_stmt(
    source: &SourceFile,
    cond: &ast::Expr,
    then_branch: &ast::Expr,
    else_branch: Option<&ast::Expr>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
    mutable_bindings: &HashSet<Span>,
    loop_depth: usize,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // smart cast（T0413）最小子集：仅识别 `if (x is T)` / `if (x !is T)` 形式，
    // 并且只对“稳定绑定”（参数 + `val`）在对应分支内做类型收窄。
    let smart_cast = detect_smart_cast_for_if_condition(cond, lower, locals, stable_bindings)?;

    // then 分支：在 `x is T` 时收窄；在 `x !is T` 时保持原类型。
    let mut then_locals = locals.clone();
    let mut then_stable = stable_bindings.clone();
    let mut then_mutable = mutable_bindings.clone();
    if let Some(smart_cast) = smart_cast {
        if smart_cast.narrow_in_then {
            then_locals.insert(smart_cast.decl_span, smart_cast.target_ty);
        }
    }
    check_expr_stmt(
        source,
        then_branch,
        lower,
        builtins,
        &mut then_locals,
        &mut then_stable,
        &mut then_mutable,
        loop_depth,
        expected_return_ty,
        top_level_types,
        top_level_funs,
        member_mutabilities,
        struct_field_types,
    )?;

    // else 分支：在 `x !is T` 且存在 else 时收窄；否则保持原类型。
    if let Some(else_branch) = else_branch {
        let mut else_locals = locals.clone();
        let mut else_stable = stable_bindings.clone();
        let mut else_mutable = mutable_bindings.clone();
        if let Some(smart_cast) = smart_cast {
            if !smart_cast.narrow_in_then {
                else_locals.insert(smart_cast.decl_span, smart_cast.target_ty);
            }
        }

        check_expr_stmt(
            source,
            else_branch,
            lower,
            builtins,
            &mut else_locals,
            &mut else_stable,
            &mut else_mutable,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        )?;
    }

    Ok(())
}

fn check_assign_expr_stmt(
    source: &SourceFile,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
    mutable_bindings: &HashSet<Span>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // T0443：赋值语句 `lhs = rhs` 最小规则：
    // - lhs 必须是可写目标：局部 `var` 绑定 或 可写属性（`var` property / ctor `var` param）
    // - rhs 类型必须可赋给 lhs（复用 `is_type_assignable` 的最小子类型/boxing 规则）
    let expected_ty = match &lhs.kind {
        ast::ExprKind::Ident(id) => {
            let Some(resolved) = id.resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（unresolved ident）",
                    span: id.span.into(),
                });
            };

            match resolved {
                ast::ResolvedValueRef::Local { name, decl_span } => {
                    if stable_bindings.contains(decl_span) || !mutable_bindings.contains(decl_span)
                    {
                        return Err(ExprTypeError::AssignmentTargetNotMutable {
                            name: name.clone(),
                            span: id.span.into(),
                        });
                    }

                    let expected_ty = locals.get(decl_span).copied().ok_or_else(|| {
                        ExprTypeError::UnknownLocalValueType {
                            name: name.clone(),
                            span: id.span.into(),
                        }
                    })?;

                    expected_ty
                }
                ast::ResolvedValueRef::TopLevel { .. } => {
                    // 目标（T0443）：先只支持局部 var（顶层 var 赋值后续再补齐）。
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "assignment lhs（top-level value）",
                        span: id.span.into(),
                    });
                }
            }
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            // 先递归 typecheck receiver：保证 `a().b = rhs` 能覆盖 `a()`。
            //
            // 例外：`TypeName.member` 经 companion object 解析时，receiver 不是值表达式；
            // resolver 会保留 receiver ident 为未解析，此处跳过 receiver typecheck。
            let receiver_is_type_name =
                matches!(&receiver.kind, ast::ExprKind::Ident(id) if id.resolved.is_none());
            if !receiver_is_type_name {
                let _ = infer_expr_type(
                    source,
                    receiver,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
            }

            let Some(resolved) = member.resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（member 未 resolve）",
                    span: member.span.into(),
                });
            };

            let fqn = match resolved {
                ast::ResolvedMemberRef::Value { fqn } => fqn,
                ast::ResolvedMemberRef::Fun { fqn }
                | ast::ResolvedMemberRef::ExtensionValue { fqn }
                | ast::ResolvedMemberRef::ExtensionFun { fqn } => {
                    return Err(ExprTypeError::UnsupportedMemberAccess {
                        fqn: fqn.clone(),
                        span: member.span.into(),
                    });
                }
            };

            if !member_mutabilities.get(fqn).copied().unwrap_or(false) {
                return Err(ExprTypeError::AssignmentTargetNotMutable {
                    name: source.slice(member.span).to_string(),
                    span: member.span.into(),
                });
            }

            let expected_ty = struct_field_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedMemberAccess {
                    fqn: fqn.clone(),
                    span: member.span.into(),
                }
            })?;

            expected_ty
        }
        _ => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "assignment lhs（仅支持标识符或成员访问）",
                span: lhs.span.into(),
            });
        }
    };

    // 递归 typecheck rhs：保证 `x = f()` 这类语句也会覆盖 rhs 中的表达式。
    let expected_from = match &lhs.kind {
        ast::ExprKind::Ident(id) => {
            ExpectedTypeFrom::new(format!("赋值目标 `{}` 的类型", source.slice(id.span)))
        }
        ast::ExprKind::MemberAccess { member, .. } => ExpectedTypeFrom::new(format!(
            "赋值目标 `{}` 的字段类型",
            source.slice(member.span)
        )),
        _ => ExpectedTypeFrom::new("赋值目标的类型"),
    };
    let found_ty = infer_expr_type_in_expected_context(
        source,
        rhs,
        expected_ty,
        expected_from,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
        // 与 initializer/call args 一致：允许整数字面量被上下文整数类型吸收（后续可加入 range check）。
        if matches!(rhs.kind, ast::ExprKind::IntLit)
            && is_integer_type(expected_ty, lower, builtins)
        {
            return Ok(());
        }
        return Err(ExprTypeError::AssignmentTypeMismatch {
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: rhs.span.into(),
        });
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct SmartCastHint {
    decl_span: Span,
    target_ty: TypeId,
    narrow_in_then: bool,
}

fn detect_smart_cast_for_if_condition(
    cond: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
) -> Result<Option<SmartCastHint>, ExprTypeError> {
    let ast::ExprKind::TypeCheck { expr, op, ty, .. } = &cond.kind else {
        return Ok(None);
    };

    let ast::ExprKind::Ident(id) = &expr.kind else {
        return Ok(None);
    };

    let Some(ast::ResolvedValueRef::Local { decl_span, .. }) = id.resolved.as_ref() else {
        return Ok(None);
    };

    if !stable_bindings.contains(decl_span) {
        return Ok(None);
    }

    let Some(from_ty) = locals.get(decl_span).copied() else {
        return Ok(None);
    };

    let target_ty = lower.lower_type_ref(ty)?;

    // spec §4.3：smart cast 只对引用类型生效（值类型使用 enum/pattern 进行收窄）。
    if !(lower.is_ref(from_ty) && lower.is_ref(target_ty)) {
        return Ok(None);
    }

    Ok(Some(SmartCastHint {
        decl_span: *decl_span,
        target_ty,
        narrow_in_then: matches!(op, ast::TypeCheckOp::Is),
    }))
}

fn expr_kind_name(kind: &ast::ExprKind) -> &'static str {
    match kind {
        ast::ExprKind::Missing => "missing",
        ast::ExprKind::Ident(_) => "ident",
        ast::ExprKind::IntLit => "int literal",
        ast::ExprKind::StringLit => "string literal",
        ast::ExprKind::UnitLit => "unit literal",
        ast::ExprKind::TupleLit { .. } => "tuple literal",
        ast::ExprKind::InterpolatedString { .. } => "interpolated string",
        ast::ExprKind::Block(_) => "block",
        ast::ExprKind::Lambda(_) => "lambda",
        ast::ExprKind::StructLit { .. } => "struct literal",
        ast::ExprKind::If { .. } => "if expression",
        ast::ExprKind::When { .. } => "when expression",
        ast::ExprKind::Handle { .. } => "handle expression",
        ast::ExprKind::Async { .. } => "async expression",
        ast::ExprKind::Spawn { .. } => "spawn expression",
        ast::ExprKind::Await { .. } => "await expression",
        ast::ExprKind::Join { .. } => "join expression",
        ast::ExprKind::MemberAccess { .. } => "member access",
        ast::ExprKind::SpliceField { .. } => "splice field access",
        ast::ExprKind::SafeMemberAccess { .. } => "safe member access",
        ast::ExprKind::Call { .. } => "call",
        ast::ExprKind::NamedArg { .. } => "named argument",
        ast::ExprKind::NotNullAssert { .. } => "not-null assertion",
        ast::ExprKind::Unary { .. } => "unary expression",
        ast::ExprKind::Binary { .. } => "binary expression",
        ast::ExprKind::Assign { .. } => "assignment",
        ast::ExprKind::TypeCheck { .. } => "type check (`is`/`!is`)",
        ast::ExprKind::Cast { .. } => "cast (`as`/`as?`)",
        ast::ExprKind::WithUpdate { .. } => "with-update",
    }
}

fn infer_safe_member_access_expr_type(
    source: &SourceFile,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先递归类型检查 receiver：保证其中的表达式也会被覆盖。
    let receiver_ty = infer_expr_type(
        source,
        receiver,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let inner_ty = match lower.type_kind(receiver_ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
        _ => {
            return Err(ExprTypeError::SafeAccessReceiverNotNullable {
                found: lower.fmt_type(receiver_ty),
                span: receiver.span.into(),
            });
        }
    };

    // 当前阶段最小规则：
    // - 仅支持 safe-call 的字段访问：`receiver?.field`，并且 field 必须是 struct 字段（T0408）。
    // - extension property / method 的语义留给后续任务；safe-call 的“调用”形式在 `Call(SafeMemberAccess)`
    //   分支中处理。
    let field_ty = match member.resolved.as_ref() {
        Some(ast::ResolvedMemberRef::Value { fqn }) => struct_field_types
            .get(fqn)
            .copied()
            .ok_or_else(|| ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            })?,
        Some(ast::ResolvedMemberRef::Fun { fqn })
        | Some(ast::ResolvedMemberRef::ExtensionValue { fqn })
        | Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) => {
            return Err(ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            });
        }
        None => {
            // resolver 无法静态确定 receiver 类型（例如 receiver 为 `T?`）时不会写回 resolved；
            // 这里用“已推导出的 inner_ty”尝试补上最小字段查找。
            let name = source.slice(member.span);
            match lower.type_kind(inner_ty) {
                TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                    let fqn = format!("{}.{}", nominal.fqn, name);
                    struct_field_types.get(&fqn).copied().ok_or_else(|| {
                        ExprTypeError::UnsupportedMemberAccess {
                            fqn,
                            span: member.span.into(),
                        }
                    })?
                }
                other => {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: match other {
                            TypeKind::Value(_) => "safe member access（非 struct 字段）",
                            TypeKind::Ref(_) => "safe member access（引用类型成员尚未支持）",
                            TypeKind::Param(_) => "safe member access（type param 暂不支持）",
                        },
                        span: member.span.into(),
                    });
                }
            }
        }
    };

    Ok(lower.ty_option(field_ty))
}

fn infer_elvis_expr_type(
    source: &SourceFile,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let lhs_ty = infer_expr_type(
        source,
        lhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let inner_ty = match lower.type_kind(lhs_ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
        _ => {
            return Err(ExprTypeError::ElvisLhsNotNullable {
                found: lower.fmt_type(lhs_ty),
                span: lhs.span.into(),
            });
        }
    };

    let rhs_ty = infer_expr_type(
        source,
        rhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if !is_type_assignable(rhs_ty, inner_ty, lower, builtins) {
        return Err(ExprTypeError::ElvisRhsTypeMismatch {
            expected: lower.fmt_type(inner_ty),
            found: lower.fmt_type(rhs_ty),
            span: rhs.span.into(),
        });
    }

    Ok(inner_ty)
}

fn infer_not_null_assert_expr_type(
    source: &SourceFile,
    expr: &ast::Expr,
    op_span: Span,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // T0421a：最小规则：
    // - `x!!` 的操作数必须是 nullable（`T?` / `Option<T>`）
    // - 结果类型为去掉 nullable 后的 inner type：`Option<T>` → `T`
    //
    // T0421b：`x!!` 的失败语义要求 `Raise<RuntimeError>`（静态 required effects）。
    let ty = infer_expr_type(
        source,
        expr,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let runtime_error = lower.lower_type_fqn_with_args(
                "scoop.core.RuntimeError".to_string(),
                Vec::new(),
                op_span,
            )?;
            let raise_runtime_error = lower.lower_type_fqn_with_args(
                "scoop.core.Raise".to_string(),
                vec![runtime_error],
                op_span,
            )?;
            lower.record_performed_effect(raise_runtime_error, op_span);
            Ok(inner)
        }
        _ => Err(ExprTypeError::NotNullAssertOperandNotNullable {
            found: lower.fmt_type(ty),
            span: expr.span.into(),
        }),
    }
}

fn infer_member_access_expr_type(
    source: &SourceFile,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // enum unit variant 值：`EnumName.Variant`（例如 `RuntimeError.NullAssertionFailed`）。
    //
    // 说明：
    // - resolver 会把该 member access 直接解析为一个 value FQN：`EnumFqn.Variant`；
    // - receiver（`EnumName`）在语义上只是“值命名空间的入口”，并非真正的运行期值；
    // - 因此这里在 typecheck 阶段直接返回 enum 类型，并跳过 receiver 的表达式 typecheck，
    //   避免把 enum type name 当作普通顶层值进行推导而报错（`UnsupportedTopLevelValueType`）。
    if let Some(ast::ResolvedMemberRef::Value { fqn }) = member.resolved.as_ref() {
        if let Some((enum_fqn, variant_name)) = fqn.rsplit_once('.') {
            if let Some(decl) = lower.env().enum_decl(enum_fqn) {
                if decl
                    .variants
                    .iter()
                    .any(|v| v.name == variant_name && v.fields.is_empty())
                {
                    return Ok(
                        lower.lower_type_fqn_with_args(enum_fqn.to_string(), Vec::new(), member.span)?
                    );
                }
            }
        }
    }

    // 先递归类型检查 receiver：保证其中的表达式（如 `a().b` 的 `a()`）也会被覆盖，
    // 并在需要时为 tuple 元素访问提供 receiver 类型信息。
    //
    // 例外：`TypeName.member` 的 companion member access 中，receiver 可能不是一个“值表达式”，
    // resolver 会刻意保留 `Ident` 的未解析状态；此时跳过 receiver typecheck。
    let receiver_is_type_name =
        matches!(&receiver.kind, ast::ExprKind::Ident(id) if id.resolved.is_none());
    let receiver_ty = if receiver_is_type_name {
        None
    } else {
        Some(infer_expr_type(
            source,
            receiver,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?)
    };

    match member.resolved.as_ref() {
        None => {
            // tuple 元素访问（spec §2.3.3）：`t._0` / `t._1` / ...
            //
            // 说明：
            // - tuple 并非名义类型，因此 resolver 阶段无法像 `Point.x` 一样写回成员 FQN；
            // - 这里在 typecheck 阶段通过 receiver 的推导类型来支持最小 tuple 元素访问语义。
            let Some(receiver_ty) = receiver_ty else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "member access（未 resolve）",
                    span: member.span.into(),
                });
            };

            let TypeKind::Value(ValueTypeKind::Tuple(elements)) = lower.type_kind(receiver_ty)
            else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "member access（未 resolve）",
                    span: member.span.into(),
                });
            };

            let member_name = source.slice(member.span);
            let Some(idx) = parse_tuple_member_index(member_name) else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "member access（未 resolve）",
                    span: member.span.into(),
                });
            };

            let ty = elements.get(idx).copied().ok_or_else(|| ExprTypeError::UnsupportedExpr {
                kind: "tuple element access（index out of bounds）",
                span: member.span.into(),
            })?;
            Ok(ty)
        }
        Some(ast::ResolvedMemberRef::Value { fqn }) => {
            // `TypeName.NestedObject` / `Obj.NestedObject`：成员本身是一个 object 单例值。
            if lower.is_object_type(fqn) {
                return Ok(lower.lower_type_fqn_with_args(fqn.clone(), Vec::new(), member.span)?);
            }

            struct_field_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedMemberAccess {
                    fqn: fqn.clone(),
                    span: member.span.into(),
                }
            })
        }
        Some(
            ast::ResolvedMemberRef::Fun { fqn }
            | ast::ResolvedMemberRef::ExtensionValue { fqn }
            | ast::ResolvedMemberRef::ExtensionFun { fqn },
        ) => {
            Err(ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            })
        }
    }
}

fn parse_tuple_member_index(text: &str) -> Option<usize> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok()
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

/// 收集当前文件内“可通过成员访问写入”的 value members 可变性（member FQN → is_var）。
///
/// 说明：
/// - 该表用于赋值语句 `lhs = rhs` 的 lhs 可写性检查（T0443）；
/// - 目前我们只在单文件内收集（typecheck fixtures 的编译单元即“sysroot + 单文件”）；
/// - 仅覆盖 struct/class 的字段/属性声明（与 `collect_struct_field_types` 的 key 集合保持一致）。
fn collect_member_mutabilities(source: &SourceFile, file: &ast::File) -> HashMap<String, bool> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, bool> = HashMap::new();

    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_member_mutabilities_in_type_decl(source, ty, &pkg_prefix, &mut map);
            }
            ast::Item::Object(obj) => {
                collect_member_mutabilities_in_object_decl(source, obj, &pkg_prefix, &mut map);
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }

    map
}

fn collect_member_mutabilities_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    out: &mut HashMap<String, bool>,
) {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, matches!(p.kind, Some(ast::ValKind::Var)));
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(p) = member else {
                    continue;
                };
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, matches!(p.kind, ast::ValKind::Var));
            }
        }
    }

    if matches!(decl.kind, ast::TypeKind::Class) {
        // class ctor `val/var` 参数声明同名字段/属性；裸参数不应进入 member 表。
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let Some(kind) = p.kind else {
                    continue;
                };
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, matches!(kind, ast::ValKind::Var));
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(p) = member else {
                    continue;
                };
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, matches!(p.kind, ast::ValKind::Var));
            }
        }
    }

    // 无论外层是否 struct/class，都递归收集 nested type（可能存在 nested struct/class）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    collect_member_mutabilities_in_type_decl(source, nested, &type_fqn, out);
                }
                ast::TypeMember::Object(obj) => {
                    collect_member_mutabilities_in_object_decl(source, obj, &type_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }
}

fn collect_member_mutabilities_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    out: &mut HashMap<String, bool>,
) {
    let obj_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => {
                // parser 会拒绝 `object { ... }` 这类非法语法；这里作为防御性兜底忽略。
                return;
            }
        },
    };

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    let Some(body) = &obj.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(p) => {
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{obj_fqn}.{field_name}");
                out.insert(field_fqn, matches!(p.kind, ast::ValKind::Var));
            }
            ast::TypeMember::Type(nested) => {
                collect_member_mutabilities_in_type_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_member_mutabilities_in_object_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

/// 收集当前文件内“可通过成员访问读取”的 value members 声明类型（member FQN → TypeId）。
///
/// 说明：
/// - 初始版本（T0408）仅收集 `struct`（值类型）的字段；
/// - T0438 起额外收集 class 的 ctor `val/var` 参数与 type body 属性，用于最小 member access typecheck；
/// - 字段来源：
///   - 主构造参数（`struct Point(val x: Int)`）：在语义上等价于字段
///   - type body 内的 `val/var` property（`struct Point { val x: Int }`）
/// - 当前阶段只在单文件内查找（typecheck fixtures 的编译单元即“sysroot + 单文件”）。
fn collect_struct_field_types(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
) -> Result<HashMap<String, TypeId>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, TypeId> = HashMap::new();

    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_struct_field_types_in_type_decl(source, ty, &pkg_prefix, lower, &mut map)?;
            }
            ast::Item::Object(obj) => {
                collect_struct_field_types_in_object_decl(
                    source,
                    obj,
                    &pkg_prefix,
                    lower,
                    &mut map,
                )?;
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }

    Ok(map)
}

fn collect_struct_field_types_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                if let ast::TypeMember::Property(p) = member {
                    let Some(ty_ref) = &p.ty else {
                        continue;
                    };
                    let field_name = source.slice(p.name.span);
                    let field_fqn = format!("{type_fqn}.{field_name}");
                    out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
                }
            }
        }
    }

    if matches!(decl.kind, ast::TypeKind::Class) {
        // class ctor `val/var` 参数声明同名字段/属性；裸参数不应进入 member 类型表。
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                if p.kind.is_none() {
                    continue;
                }
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                if let ast::TypeMember::Property(p) = member {
                    let Some(ty_ref) = &p.ty else {
                        continue;
                    };
                    let field_name = source.slice(p.name.span);
                    let field_fqn = format!("{type_fqn}.{field_name}");
                    out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
                }
            }
        }
    }

    // 无论外层是否 struct，都递归收集 nested type（可能存在 nested struct）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    collect_struct_field_types_in_type_decl(source, nested, &type_fqn, lower, out)?;
                }
                ast::TypeMember::Object(obj) => {
                    collect_struct_field_types_in_object_decl(source, obj, &type_fqn, lower, out)?;
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

fn collect_struct_field_types_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let obj_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => {
                // parser 会拒绝 `object { ... }` 这类非法语法；这里作为防御性兜底忽略。
                return Ok(());
            }
        },
    };

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(p) => {
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{obj_fqn}.{field_name}");
                out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
            }
            ast::TypeMember::Type(nested) => {
                collect_struct_field_types_in_type_decl(source, nested, &obj_fqn, lower, out)?;
            }
            ast::TypeMember::Object(nested) => {
                collect_struct_field_types_in_object_decl(source, nested, &obj_fqn, lower, out)?;
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;

    #[test]
    fn nothing_is_assignable_to_any_type() {
        // 该测试不依赖 sysroot 或 resolver 的完整能力；
        // 只验证 typecheck 的“赋值兼容”最小规则：`Nothing <: T`。
        let source = SourceFile::new_virtual("<mem>", "package a\nfun f(): Unit { return }");
        let file = parse_file(&source).unwrap();
        let index = Index::build(&[(&source, &file)]).unwrap();
        let imports = ImportTable::build(&source, &file, &index).unwrap();

        let env = TypeEnv::default();
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let lower = TypeLowering::new(&source, &file, &index, &imports, &env, &mut types, builtins);

        assert!(is_type_assignable(
            builtins.nothing,
            builtins.any,
            &lower,
            builtins
        ));
        assert!(is_type_assignable(
            builtins.nothing,
            builtins.unit,
            &lower,
            builtins
        ));
        assert!(is_type_assignable(
            builtins.nothing,
            builtins.bool_,
            &lower,
            builtins
        ));

        // 反例：普通值类型不应在 v0 阶段隐式互转。
        assert!(!is_type_assignable(
            builtins.unit,
            builtins.bool_,
            &lower,
            builtins
        ));
    }
}
