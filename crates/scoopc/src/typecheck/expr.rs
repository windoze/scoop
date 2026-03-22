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
use crate::infer::{InferError, InferTerm, Solver};
use crate::resolve::{ConeId, ConstructorOverload, ImportTable, Index, Visibility};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::assignable::{is_type_assignable, nominal_is_subtype_by_fqn};
use super::lower::{TypeLowerError, TypeLowering};
use super::val_pat;
use super::when_exhaustiveness;
use super::when_pat;
use super::{TypeEnv, TypeSymbolKind, type_env::EnumVariantInfo};

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

    #[error("重载决议歧义：{callee}")]
    #[diagnostic(code(scoop::typecheck::ambiguous_overload))]
    AmbiguousOverload {
        callee: String,
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

    #[error("无法推断泛型类型实参：{callee} 的 `{param}`")]
    #[diagnostic(code(scoop::typecheck::generic_type_arg_not_inferred))]
    GenericTypeArgNotInferred {
        callee: String,
        param: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("泛型类型实参推断冲突：{callee} 的 `{param}` 同时被约束为 {left} 与 {right}")]
    #[diagnostic(code(scoop::typecheck::generic_type_arg_inference_conflict))]
    GenericTypeArgInferenceConflict {
        callee: String,
        param: String,
        left: String,
        right: String,
        #[label("这里")]
        span: miette::SourceSpan,
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
struct FunSigOwned {
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
    /// 函数级 type params（按声明顺序）。
    ///
    /// 用途（T0505）：
    /// - 让调用点可以识别“哪些 TypeId 是该函数的类型参数”
    /// - 在参数检查前做最小泛型实参推断，并对签名做 substitution（实例化）
    type_params: Vec<TypeId>,
    params: Vec<TypeId>,
    return_ty: TypeId,
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
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
    // 这里单独拷贝一份 package 前缀，避免在借用 `lower` 的同时再借用其字段导致借用冲突。
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    // 顶层 `val/var` 的类型表：用于在表达式里引用顶层变量时查询其声明类型。
    //
    // 当前阶段约束：
    // - 只支持“当前文件内”的顶层变量（因为 typecheck phase 目前只解析单文件 AST）；
    // - 顶层变量必须有显式类型注解（由 `typecheck::check_file_headers` 保证）。
    let top_level_types = collect_top_level_value_types(source, file, &mut lower)?;
    let top_level_funs = collect_top_level_fun_signatures(source, file, &mut lower, builtins)?;
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
            ast::Item::Fun(fun) => check_fun_body_exprs(
                source,
                fun,
                &mut lower,
                builtins,
                &top_level_types,
                &top_level_funs,
                &member_mutabilities,
                &struct_field_types,
            )?,
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

    Ok(())
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
    let result: Result<(), ExprTypeError> = (|| {
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
            None => builtins.unit,
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
    let found = infer_expr_type_in_expected_context(
        source,
        init,
        expected,
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
        ast::ExprKind::NotNullAssert { expr: inner, .. } => infer_not_null_assert_expr_type(
            source,
            inner.as_ref(),
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
                ast::CastOp::As => Ok(target_ty),
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
            // `when` 表达式结果类型：当前阶段（T0414）只实现最小规则：
            // - 递归类型检查 subject 与每个 arm body（保证覆盖其中的表达式）；
            // - 若所有分支 body 的类型相同，则结果为该类型；
            // - 若存在多个非 `Nothing` 的分支且类型不一致，则 fallback 为 `Any`（真正的 LUB 规则留到后续任务实现）。
            // - 若所有分支都是 `Nothing`（不可达），则整体结果为 `Nothing`。
            //
            // 额外：`Nothing` 是 bottom type（T0420a），因此：
            // - `Nothing` 与任意 `T` 的 LUB 至少应为 `T`；
            // - 这里先做一个最小 special-case：忽略 `Nothing` 分支参与比较。
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

                // `Nothing`：不可达分支（例如后续 `Raise.raise`），不影响最小 LUB 推导。
                if arm_ty == builtins.nothing {
                    continue;
                }

                match result {
                    None => result = Some(arm_ty),
                    Some(prev) if prev == arm_ty => {}
                    Some(_) => result = Some(builtins.any),
                }
            }

            when_exhaustiveness::check_when_exhaustiveness(
                source, expr, subject_ty, arms, lower, builtins,
            )?;

            // 若所有分支都是 `Nothing`，则 `when` 整体也是不可达的。
            Ok(result.unwrap_or(builtins.nothing))
        }
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
    // 当前阶段（T0502）仅把 `if` 当作“可推导结果类型”的表达式，
    // 以支持 `val x = if (...) 1 else 2` 这类最小推断回归。
    //
    // 非目标（后续任务 T0514）：真正的 LUB / union 规则与更强的条件类型约束。

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

    let then_ty = infer_expr_type(
        source,
        then_branch,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let Some(else_branch) = else_branch else {
        // `if` 没有 else：语义上更接近“语句形式”，结果类型视为 `Unit`。
        // 仍然需要确保 then branch 内的表达式被覆盖（见上方 `then_ty`）。
        return Ok(builtins.unit);
    };

    let else_ty = infer_expr_type(
        source,
        else_branch,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // `Nothing` 是 bottom type：与任意 `T` 合并时应选择 `T`（最小 special-case）。
    if then_ty == builtins.nothing {
        return Ok(else_ty);
    }
    if else_ty == builtins.nothing {
        return Ok(then_ty);
    }

    if then_ty == else_ty {
        return Ok(then_ty);
    }

    // TODO(T0514): 这里先用 `Any` 作为最小 fallback，后续用真正的 LUB/union 替换。
    Ok(builtins.any)
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

    // 当前阶段目标（T0504）：先只支持“单参数 lambda”，并且不支持 receiver function type。
    if expected_fun.receiver.is_some() || expected_fun.params.len() != 1 {
        return Ok(None);
    }

    if lam.params.len() != 1 {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "lambda（当前仅支持单参数，且参数类型需来自期望函数类型）",
            span: lam_expr.span.into(),
        });
    }

    let expected_param_ty = expected_fun.params[0];
    let param = &lam.params[0];
    let param_ty = match &param.ty {
        Some(ty_ref) => lower.lower_type_ref(ty_ref)?,
        None => expected_param_ty,
    };

    // lambda 内部的作用域（最小子集）：
    // - 继承外层捕获的局部绑定
    // - 注入 lambda 形参（供 body 的 ident typecheck 使用）
    let mut lambda_locals = locals.clone();
    lambda_locals.insert(param.name.span, param_ty);

    // 返回类型推导（最小）：以 body 表达式的类型为 lambda 返回类型。
    // 当前阶段不做“expected return type 向下传播”（避免引入多段推断链）。
    let body_ty = infer_expr_type(
        source,
        lam.body.as_ref(),
        lower,
        builtins,
        &lambda_locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let lam_ty = lower.ty_function(None, vec![param_ty], body_ty, EffectRow::pure());
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
                ast::ResolvedValueRef::Local { .. } => {
                    // 当前阶段（T0407）只支持直接调用“顶层 fun symbol”，
                    // 不支持通过值调用（函数值/闭包等）。
                    return Err(ExprTypeError::CalleeNotCallable {
                        callee: callee_name.to_string(),
                        span: id.span.into(),
                    });
                }
            };

            // 当前阶段仅支持“当前文件内”的顶层函数调用类型检查（无重载、无默认参数）。
            let Some(sigs) = top_level_funs.get(&callee_fqn) else {
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
                if args.len() != sig.params.len() {
                    return Err(ExprTypeError::CallArityMismatch {
                        callee: callee_fqn,
                        expected: sig.params.len(),
                        found: args.len(),
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
                let Some(mapping) = map_call_args_to_params(&call_args, &sig.param_names) else {
                    return Err(ExprTypeError::NoMatchingOverload {
                        callee: callee_fqn,
                        span: call_expr.span.into(),
                    });
                };

                let instantiated = instantiate_fun_sig_for_call(
                    &callee_fqn,
                    call_expr.span,
                    sig,
                    mapping
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(param_idx, arg_idx)| {
                            let arg = &call_args[arg_idx];
                            GenericArgConstraint {
                                expected: sig.params[param_idx],
                                found: arg.ty,
                                found_is_placeholder: matches!(
                                    arg.expr.kind,
                                    ast::ExprKind::Lambda(_)
                                ),
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
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?;

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

                return Ok(instantiated.return_ty);
            }

            // 多候选：执行最小 overload resolution（T0453）。
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

            let mut matched: Vec<&FunSigOwned> = Vec::new();
            for cand in direct_call_candidates {
                if call_args.len() != cand.params.len() {
                    continue;
                }

                let Some(mapping) = map_call_args_to_params(&call_args, &cand.param_names) else {
                    continue;
                };

                let mut ok = true;
                for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                    let arg = &call_args[arg_idx];
                    let expected_ty = cand.params[param_idx];
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
                    matched.push(cand);
                }
            }

            match matched.len() {
                0 => Err(ExprTypeError::NoMatchingOverload {
                    callee: callee_fqn,
                    span: call_expr.span.into(),
                }),
                1 => Ok(matched[0].return_ty),
                _ => Err(ExprTypeError::AmbiguousOverload {
                    callee: callee_fqn,
                    span: call_expr.span.into(),
                }),
            }
        }
        ast::ExprKind::MemberAccess { receiver, member } => infer_member_call_expr_type(
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
        ),
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

    let mut matched_ctor_count = 0usize;
    let mut matched_type_fqn: Option<String> = None;

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
                matched_ctor_count += 1;
                matched_type_fqn = Some(ty_fqn.clone());
                if matched_ctor_count > 1 {
                    break;
                }
            }
        }

        if matched_ctor_count > 1 {
            break;
        }
    }

    match matched_ctor_count {
        0 => Err(ExprTypeError::NoMatchingOverload {
            callee: callee_name,
            span: call_expr.span.into(),
        }),
        1 => {
            let ty_fqn = matched_type_fqn.expect("matched_ctor_count == 1");
            let ty = lower.lower_type_fqn_with_args(ty_fqn, Vec::new(), callee.span)?;
            Ok(Some(ty))
        }
        _ => Err(ExprTypeError::AmbiguousOverload {
            callee: callee_name,
            span: call_expr.span.into(),
        }),
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
        let expected_args = sig.params.len().saturating_sub(1);
        if call_args.len() != expected_args {
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

        let Some(mapping) = map_call_args_to_params(&call_args, param_names) else {
            return Err(ExprTypeError::NoMatchingOverload {
                callee: callee_fqn,
                span: call_expr.span.into(),
            });
        };

        let instantiated =
            instantiate_fun_sig_for_call(
                &callee_fqn,
                call_expr.span,
                sig,
                std::iter::once(GenericArgConstraint {
                    expected: expected_receiver_ty,
                    found: actual_receiver_ty,
                    found_is_placeholder: false,
                })
                .chain(mapping.iter().copied().enumerate().map(
                    |(param_idx, arg_idx)| {
                        let arg = &call_args[arg_idx];
                        GenericArgConstraint {
                            expected: sig.params[param_idx + 1],
                            found: arg.ty,
                            found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                        }
                    },
                )),
                lower,
                builtins,
            )?;

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

        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &call_args[arg_idx];
            let found_ty = infer_expr_type_in_expected_context(
                source,
                arg.expr,
                expected_ty,
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
                // extension 调用：`receiver.member(arg1, arg2, ...)` 的第 1 个“显式参数”
                // 对应 `sig.params[1]`（跳过 receiver 参数）。
                index: param_idx + 1,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: arg.expr.span.into(),
            });
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
        receiver_ty: TypeId,
        /// `call_args[arg_idx]` 对应的“期望类型”（排除了 receiver 参数）。
        expected_arg_tys: Vec<TypeId>,
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

    fn pick_most_specific_extension_overload<'a>(
        candidates: &[MatchedExtensionOverload<'a>],
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> Option<&'a FunSigOwned> {
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
                return Some(cand.sig);
            }
        }
        None
    }

    // 多候选：先按 receiver/参数匹配筛选，再用 receiver/参数 specificity 选出 most-specific（T0455）。
    let mut matched: Vec<MatchedExtensionOverload<'_>> = Vec::new();

    for cand in ext_candidates {
        let Some(expected_receiver_ty) = cand.params.first().copied() else {
            continue;
        };
        if !is_type_assignable(actual_receiver_ty, expected_receiver_ty, lower, builtins) {
            continue;
        }

        let expected_args = cand.params.len().saturating_sub(1);
        if call_args.len() != expected_args {
            continue;
        }

        let Some(param_names) = cand.param_names.get(1..) else {
            continue;
        };
        let Some(mapping) = map_call_args_to_params(&call_args, param_names) else {
            continue;
        };

        let mut ok = true;
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let expected_ty = cand.params[param_idx + 1];
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
            let mut expected_arg_tys = vec![builtins.nothing; call_args.len()];
            for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
                expected_arg_tys[arg_idx] = cand.params[param_idx + 1];
            }

            matched.push(MatchedExtensionOverload {
                sig: cand,
                receiver_ty: expected_receiver_ty,
                expected_arg_tys,
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
        1 => matched[0].sig,
        _ => pick_most_specific_extension_overload(&matched, lower, builtins).ok_or_else(|| {
            ExprTypeError::AmbiguousOverload {
                callee: callee_fqn,
                span: call_expr.span.into(),
            }
        })?,
    };

    let ret = if safe {
        lower.ty_option(chosen.return_ty)
    } else {
        chosen.return_ty
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

        // fun 自身的 type params 在签名 lowering 语境内可见。
        lower.push_type_params(&fun.type_params);
        let result: Result<(), ExprTypeError> = (|| {
            let type_params: Vec<TypeId> = fun
                .type_params
                .iter()
                .map(|p| lower.ty_param_from_decl(p))
                .collect();

            let mut param_names = Vec::with_capacity(fun.params.len() + 1);
            let mut params = Vec::with_capacity(fun.params.len() + 1);

            // spec §7.4：扩展函数编译为普通静态函数：receiver 作为第一个参数。
            // typecheck 阶段也沿用这一“降糖”形式，便于统一调用检查逻辑。
            let is_extension = fun.receiver.is_some();
            let is_inline = fun.modifiers.contains(&ast::Modifier::Inline);
            if let Some(receiver) = &fun.receiver {
                // receiver 本身没有名字；这里用占位符保持与 `params` 对齐。
                param_names.push("<receiver>".to_string());
                params.push(lower.lower_type_ref(receiver)?);
            }

            for p in &fun.params {
                let Some(ty_ref) = &p.ty else {
                    // headers check 已保证参数类型注解存在；这里保持健壮性。
                    continue;
                };
                param_names.push(source.slice(p.name.span).to_string());
                params.push(lower.lower_type_ref(ty_ref)?);
            }

            let return_ty = match &fun.return_ty {
                Some(ret) => lower.lower_type_ref(ret)?,
                None => builtins.unit,
            };

            map.entry(fqn).or_default().push(FunSigOwned {
                is_extension,
                is_inline,
                param_names,
                type_params,
                params,
                return_ty,
            });
            Ok(())
        })();
        lower.pop_type_params(&fun.type_params);
        result?;
    }

    Ok(map)
}

#[derive(Debug, Clone)]
struct InstantiatedFunSig {
    params: Vec<TypeId>,
    return_ty: TypeId,
}

#[derive(Debug, Clone, Copy)]
struct GenericArgConstraint {
    expected: TypeId,
    found: TypeId,
    /// 若为 `true`，表示 `found` 只是“为了 overload 筛选占位”的类型（例如 lambda 在预收集阶段被记为 `Any`），
    /// 不应当用于泛型推断。
    found_is_placeholder: bool,
}

fn type_param_name(ty: TypeId, lower: &TypeLowering<'_>) -> String {
    match lower.type_kind(ty) {
        TypeKind::Param(p) => p.name,
        _ => "<type param>".to_string(),
    }
}

fn collect_eq_constraints_for_single_type_param(
    expected: TypeId,
    found: TypeId,
    param_ty: TypeId,
    infer_var: crate::infer::InferVarId,
    solver: &mut Solver,
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
        solver.eq(InferTerm::Var(infer_var), InferTerm::Ty(found));
        return;
    }

    let expected_kind = lower.type_kind(expected);
    let found_kind = lower.type_kind(found);

    match (expected_kind, found_kind) {
        (
            TypeKind::Value(ValueTypeKind::Option(expected_inner)),
            TypeKind::Value(ValueTypeKind::Option(found_inner)),
        ) => {
            collect_eq_constraints_for_single_type_param(
                expected_inner,
                found_inner,
                param_ty,
                infer_var,
                solver,
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
                collect_eq_constraints_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    infer_var,
                    solver,
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
                collect_eq_constraints_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    infer_var,
                    solver,
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
                collect_eq_constraints_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    infer_var,
                    solver,
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
                collect_eq_constraints_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    infer_var,
                    solver,
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
                collect_eq_constraints_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    infer_var,
                    solver,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }

            collect_eq_constraints_for_single_type_param(
                expected_fun.return_ty,
                found_fun.return_ty,
                param_ty,
                infer_var,
                solver,
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
            if !changed {
                return Ok(ty);
            }
            Ok(lower.lower_type_fqn_with_args(nominal.fqn, args, use_span)?)
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
            if !changed {
                return Ok(ty);
            }
            Ok(lower.lower_type_fqn_with_args(nominal.fqn, args, use_span)?)
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
    }
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

    let mut solver = Solver::new();
    let var = solver.new_var();

    for c in constraints {
        collect_eq_constraints_for_single_type_param(
            c.expected,
            c.found,
            param_ty,
            var,
            &mut solver,
            lower,
            builtins,
            c.found_is_placeholder,
        );
    }

    match solver.solve(lower.types(), builtins) {
        Ok(()) => {}
        Err(InferError::TypeConflict { left, right }) => {
            return Err(ExprTypeError::GenericTypeArgInferenceConflict {
                callee: callee.to_string(),
                param: param_name,
                left: lower.fmt_type(left),
                right: lower.fmt_type(right),
                span: call_span.into(),
            });
        }
        Err(InferError::UnsupportedConstraint { .. }) => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "generic inference constraint（internal）",
                span: call_span.into(),
            });
        }
        Err(InferError::SubtypeNotSatisfied { .. } | InferError::IncompatibleBounds { .. }) => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "generic inference constraint（subtype bounds）",
                span: call_span.into(),
            });
        }
    }

    let Some(binding) = solver.binding_of(var) else {
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

    Ok(InstantiatedFunSig { params, return_ty })
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
}

fn check_fun_body_exprs(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    lower.push_type_params(&fun.type_params);
    let result: Result<(), ExprTypeError> = (|| {
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
            None => builtins.unit,
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

    // 先类型检查 initializer（语义：局部变量在其声明之后可见，因此 init 内不能引用自身）。
    let init_ty = match &v.init {
        Some(init) => Some(match declared_ty {
            Some(expected) => infer_expr_type_in_expected_context(
                source,
                init,
                expected,
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
    let found_ty = infer_expr_type_in_expected_context(
        source,
        rhs,
        expected_ty,
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
        TypeKind::Value(ValueTypeKind::Option(inner)) => Ok(inner),
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
    let Some(resolved) = member.resolved.as_ref() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "member access（未 resolve）",
            span: member.span.into(),
        });
    };

    // 先递归类型检查 receiver：保证其中的表达式（如 `a().b` 的 `a()`）也会被覆盖。
    //
    // 例外：`TypeName.member` 的 companion member access 中，receiver 可能不是一个“值表达式”，
    // resolver 会刻意保留 `Ident` 的未解析状态；此时跳过 receiver typecheck。
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

    match resolved {
        ast::ResolvedMemberRef::Value { fqn } => {
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
        ast::ResolvedMemberRef::Fun { fqn }
        | ast::ResolvedMemberRef::ExtensionValue { fqn }
        | ast::ResolvedMemberRef::ExtensionFun { fqn } => {
            Err(ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            })
        }
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
