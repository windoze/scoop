use miette::Diagnostic;
use thiserror::Error;

use super::super::AnnotationError;
use super::super::lower::TypeLowerError;

#[derive(Debug, Error, Diagnostic)]
pub enum ExprTypeError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Annotation(#[from] AnnotationError),

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

    #[error(
        "程序入口 `main` 只能是以下四种形状之一：`fun main(): Unit / Pure!`、`fun main(): Int / Pure!`、`fun main(args: Array<String>): Unit / Pure!`、`fun main(args: Array<String>): Int / Pure!`；这里是 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::entry_point_main_invalid_signature))]
    EntryPointMainInvalidSignature {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("导出入口 `{entry}` 必须显式声明闭合 effect row：`Pure!`")]
    #[diagnostic(code(scoop::typecheck::export_entry_point_must_declare_closed_pure))]
    ExportEntryPointMustDeclareClosedPure {
        entry: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("导出入口 `{entry}` 必须为 Pure（不能声明为 {declared}）")]
    #[diagnostic(code(scoop::typecheck::export_entry_point_must_be_pure))]
    ExportEntryPointMustBePure {
        entry: String,
        declared: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "导出入口 `{entry}` 必须为闭合 effect row：`Pure!`（这里写的是 {declared}，请在 row 末尾加 `!`）"
    )]
    #[diagnostic(code(scoop::typecheck::export_entry_point_must_be_closed_pure))]
    ExportEntryPointMustBeClosedPure {
        entry: String,
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

    #[error("数组字面量元素类型不匹配：第 {index} 个元素期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::array_lit_element_type_mismatch))]
    ArrayLitElementTypeMismatch {
        index: usize,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "空数组字面量缺少可推断的元素类型；请添加显式类型标注（例如 `val xs: Array<Int> = []`）"
    )]
    #[diagnostic(code(scoop::typecheck::array_lit_type_annotation_required))]
    ArrayLitTypeAnnotationRequired {
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

    #[error("`Continuation` 是 compiler-owned interface，不能直接构造")]
    #[diagnostic(code(scoop::typecheck::continuation_not_constructible))]
    ContinuationNotConstructible {
        #[label("这里试图构造 `Continuation`")]
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

    #[error("访问 `@Extern` 顶层变量需要 unsafe context：{global}")]
    #[diagnostic(code(scoop::typecheck::extern_global_access_requires_unsafe))]
    ExternGlobalAccessRequiresUnsafeContext {
        global: String,
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

    #[error("调用函数指针需要 unsafe context：{callee}")]
    #[diagnostic(code(scoop::typecheck::funptr_call_requires_unsafe))]
    FunPtrCallRequiresUnsafeContext {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用 `{callee}` 时，形参 `var` 需要传入可寻址变量（lvalue）")]
    #[diagnostic(code(scoop::typecheck::var_param_requires_lvalue))]
    VarParamRequiresLValue {
        callee: String,
        #[label("这里需要变量")]
        span: miette::SourceSpan,
    },

    #[error("使用 unsafe 指针原语需要 unsafe context：{primitive}")]
    #[diagnostic(code(scoop::typecheck::unsafe_ptr_primitive_requires_unsafe))]
    UnsafePtrPrimitiveRequiresUnsafeContext {
        primitive: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("unsafe 指针原语 `{primitive}` 需要 `Ptr<T>` 类型，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::unsafe_ptr_primitive_requires_ptr))]
    UnsafePtrPrimitiveRequiresPtrType {
        primitive: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@NoGC` 上下文禁止调用非 `@NoGC/@Extern` 函数：{callee}")]
    #[diagnostic(code(scoop::typecheck::nogc_call_forbidden))]
    NoGcCallForbidden {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@NoGC` 上下文禁止装箱（可能堆分配）：{from} -> {to}")]
    #[diagnostic(code(scoop::typecheck::nogc_boxing_forbidden))]
    NoGcBoxingForbidden {
        from: String,
        to: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 只能调用 const fun/编译器 intrinsic：{callee}")]
    #[diagnostic(code(scoop::typecheck::const_fun_call_forbidden))]
    ConstFunCallForbidden {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 中不允许使用闭包/lambda")]
    #[diagnostic(code(scoop::typecheck::const_fun_lambda_not_allowed))]
    ConstFunLambdaNotAllowed {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 中不允许调用函数值/闭包：{callee}")]
    #[diagnostic(code(scoop::typecheck::const_fun_function_value_call_not_allowed))]
    ConstFunFunctionValueCallNotAllowed {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 中不允许调用函数指针：{callee}")]
    #[diagnostic(code(scoop::typecheck::const_fun_funptr_call_not_allowed))]
    ConstFunFunPtrCallNotAllowed {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 中禁止装箱（可能堆分配）：{from} -> {to}")]
    #[diagnostic(code(scoop::typecheck::const_fun_boxing_forbidden))]
    ConstFunBoxingForbidden {
        from: String,
        to: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 中不允许构造引用类型实例（class）：{ty}")]
    #[diagnostic(code(scoop::typecheck::const_fun_ref_type_construction_not_allowed))]
    ConstFunRefTypeConstructionNotAllowed {
        ty: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("GC.pin 当前阶段仅支持引用类型（heap/box 对象）：{found}")]
    #[diagnostic(code(scoop::typecheck::gc_pin_requires_ref))]
    GcPinRequiresRefType {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("GC.unpin 当前阶段仅支持 `Pinned` handle：{found}")]
    #[diagnostic(code(scoop::typecheck::gc_unpin_requires_ref))]
    GcUnpinRequiresRefType {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("GC.handleNew 当前阶段仅支持引用类型（heap/box 对象）：{found}")]
    #[diagnostic(code(scoop::typecheck::gc_handle_new_requires_ref))]
    GcHandleNewRequiresRefType {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("GC.handleGet 当前阶段仅支持 `GcHandle`：{found}")]
    #[diagnostic(code(scoop::typecheck::gc_handle_get_requires_handle))]
    GcHandleGetRequiresGcHandle {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("GC.handleDrop 当前阶段仅支持 `GcHandle`：{found}")]
    #[diagnostic(code(scoop::typecheck::gc_handle_drop_requires_handle))]
    GcHandleDropRequiresGcHandle {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("跨线程 resume 当前阶段只支持 `Pure` continuation，不能传播 effect row：{effects}")]
    #[diagnostic(code(scoop::typecheck::cross_thread_resume_outward_effects_unsupported))]
    CrossThreadResumeOutwardEffectsUnsupported {
        effects: String,
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

    #[error("调用 `{callee}` 缺少必需参数：{missing}")]
    #[diagnostic(code(scoop::typecheck::call_missing_required_args))]
    CallMissingRequiredArgs {
        callee: String,
        missing: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用 `{callee}` 没有名为 `{name}` 的参数")]
    #[diagnostic(code(scoop::typecheck::unknown_call_arg_name))]
    UnknownCallArgName {
        callee: String,
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用 `{callee}` 的参数 `{name}` 被重复赋值")]
    #[diagnostic(code(scoop::typecheck::call_arg_duplicate))]
    CallArgDuplicate {
        callee: String,
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用 `{callee}` 命名参数之后不能再使用位置参数")]
    #[diagnostic(code(scoop::typecheck::call_arg_positional_after_named))]
    CallArgPositionalAfterNamed {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("函数类型调用 `{callee}` 不支持命名实参")]
    #[diagnostic(
        code(scoop::typecheck::named_args_not_supported_for_callable_type),
        help("只有具名函数、方法和构造器支持命名实参；函数值、闭包和 `FunPtr<F>` 调用请使用位置实参")
    )]
    NamedArgsNotSupportedForCallableType {
        callee: String,
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

    #[error("spread 实参只能用于 vararg 形参：{callee}")]
    #[diagnostic(code(scoop::typecheck::spread_arg_requires_vararg))]
    SpreadArgRequiresVararg {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("spread 实参类型不支持：期望 Array/tuple，但得到 {found}{hint}")]
    #[diagnostic(code(scoop::typecheck::vararg_spread_requires_array_or_tuple))]
    VarargSpreadRequiresArrayOrTuple {
        found: String,
        hint: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("spread 实参的元素类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::vararg_spread_element_type_mismatch))]
    VarargSpreadElementTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("默认参数值类型不匹配：{fun} 的形参 `{param}` 期望 {expected}，但默认值为 {found}")]
    #[diagnostic(code(scoop::typecheck::default_param_value_type_mismatch))]
    DefaultParamValueTypeMismatch {
        fun: String,
        param: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "无法推断泛型类型实参：{callee} 的 `{param}`（缺少可用于推断的调用点约束；可尝试显式类型实参：`{callee}<...>(...)`）"
    )]
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
        // 该 variant 同时携带 6 个 String，会把整个 ExprTypeError 枚举体抬到
        // clippy `result_large_err` 的阈值之上。把字符串字段单独装箱后，
        // 保持诊断文本不变，但让 `Result<_, ExprTypeError>` 的 Err 载荷显著变小。
        callee: Box<String>,
        param: Box<String>,
        left: Box<String>,
        right: Box<String>,
        left_from: Box<String>,
        right_from: Box<String>,
        #[label("这里（产生冲突的约束）")]
        span: miette::SourceSpan,
        #[label("这里（之前的约束）")]
        previous: miette::SourceSpan,
    },

    #[error("显式类型实参数量不匹配：{callee} 期望 {expected} 个，但提供了 {found} 个")]
    #[diagnostic(code(scoop::typecheck::generic_type_arg_arity_mismatch))]
    GenericTypeArgArityMismatch {
        callee: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    /// 泛型函数调用处 `where` 约束不满足（T0129）。
    #[error("泛型约束不满足：{callee} 的类型实参 {arg} 不满足 {param} : {bound}")]
    #[diagnostic(code(scoop::typecheck::where_constraint_not_satisfied))]
    FunWhereConstraintNotSatisfied {
        callee: String,
        param: String,
        arg: String,
        bound: String,
        #[label("这里的类型实参不满足 where 约束")]
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

    #[error("`when` 的 Int literal pattern 只能用于整数类型，但 subject 为 {found}")]
    #[diagnostic(code(scoop::typecheck::when_int_pat_not_int))]
    WhenIntPatNotInt {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 的 String literal pattern 只能用于 String，但 subject 为 {found}")]
    #[diagnostic(code(scoop::typecheck::when_string_pat_not_string))]
    WhenStringPatNotString {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 的 Bool literal pattern 只能用于 Bool，但 subject 为 {found}")]
    #[diagnostic(code(scoop::typecheck::when_bool_pat_not_bool))]
    WhenBoolPatNotBool {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`when` 的 Char literal pattern 只能用于 Char，但 subject 为 {found}")]
    #[diagnostic(code(scoop::typecheck::when_char_pat_not_char))]
    WhenCharPatNotChar {
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

    #[error("`when` 的 variant pattern enum 前缀不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::when_variant_pat_enum_mismatch))]
    WhenVariantPatEnumMismatch {
        expected: String,
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

    #[error("splice 字段访问 `.[field]` 要求 `field` 是编译期已知的字段名")]
    #[diagnostic(code(scoop::typecheck::splice_field_name_not_static))]
    SpliceFieldNameNotStatic {
        #[label("该表达式不能静态解析为字段名")]
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

    #[error(
        "显式 `as`/`as?` 不支持函数类型的 runtime cast：{from} -> {to}；请改用函数子类型/coercion，或先包进 nominal wrapper"
    )]
    #[diagnostic(code(scoop::typecheck::function_type_cast_not_supported))]
    FunctionTypeCastNotSupported {
        from: String,
        to: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "带非 `Pure` effect row 的函数类型不能参与显式 `as`/`as?`：{from} -> {to}；effect row 只存在于编译期，不能作为 runtime cast 合同"
    )]
    #[diagnostic(code(scoop::typecheck::effectful_function_type_cast_not_supported))]
    EffectfulFunctionTypeCastNotSupported {
        from: String,
        to: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "函数值擦除到 `Any` 仅允许闭合纯函数类型 `(...)->R / Pure!`（effects 不可在运行时保真）；但这里得到 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::fn_value_to_any_requires_closed_pure))]
    FnValueToAnyRequiresClosedPure {
        found: String,
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

    #[error("操作符 `{op}` 未找到可用的重载：期望 `{receiver}.{method}({rhs})`")]
    #[diagnostic(code(scoop::typecheck::operator_overload_not_found))]
    OperatorOverloadNotFound {
        op: String,
        receiver: String,
        method: String,
        rhs: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("操作符 `{op}` 未找到可用的重载：期望 `{receiver}.{method}()`")]
    #[diagnostic(code(scoop::typecheck::unary_operator_overload_not_found))]
    UnaryOperatorOverloadNotFound {
        op: String,
        receiver: String,
        method: String,
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

    #[error("`with` 的 base 必须是可复制更新的值类型（struct/tuple/enum），但得到 {found}")]
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
        "`with` 嵌套字段路径不可继续：`{struct_name}.{field}` 的类型必须是可复制更新的值类型（struct/tuple/enum），但得到 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::with_update_nested_path_not_struct))]
    WithUpdateNestedPathNotStruct {
        struct_name: String,
        field: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`{enum_name}` 不存在 variant：{variant}")]
    #[diagnostic(code(scoop::typecheck::with_update_unknown_variant))]
    WithUpdateUnknownVariant {
        enum_name: String,
        variant: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("enum `with` 路径必须在 `{enum_name}.{variant}` 后继续指定 payload 字段")]
    #[diagnostic(code(scoop::typecheck::with_update_variant_field_required))]
    WithUpdateVariantFieldRequired {
        enum_name: String,
        variant: String,
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

    #[error("`return` 只能出现在立即包裹它的命名函数体内；lambda 中不支持 non-local return")]
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

    #[error("`for` 迭代对象类型 {found} 不支持迭代：缺少 `iterator()`")]
    #[diagnostic(code(scoop::typecheck::for_missing_iterator_method))]
    ForMissingIteratorMethod {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`for` 迭代器类型 {found} 缺少 `next()`（需返回 `Option<T>`）")]
    #[diagnostic(code(scoop::typecheck::for_missing_next_method))]
    ForMissingNextMethod {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`for` 迭代器的 `next()` 返回类型必须是 `Option<T>`（迭代协议已升级，不再使用 `hasNext()`），但得到 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::for_next_not_option))]
    ForNextNotOption {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },
}
