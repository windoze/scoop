//! typecheck 阶段的稳定诊断码与构造辅助。
//!
//! 诊断码形如 `scoop::typecheck::<name>`，需与 `tests/fixtures/typecheck/` 的
//! `EXPECT-ERROR-CODE` 对齐。

use scoop2_base::{Span, diag::Diagnostic};

/// `scoop::typecheck::type_mismatch`：期望类型与实际类型不兼容。
pub fn type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::type_mismatch",
        format!("类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::cannot_call`：表达式不可调用（非函数类型）。
pub fn cannot_call(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::cannot_call",
        format!("不可调用：{found} 不是函数类型"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::arity_mismatch`：调用实参数量与形参不符。
pub fn arity_mismatch(expected: usize, found: usize, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::arity_mismatch",
        format!("参数数量不匹配：期望 {expected} 个，但传入 {found} 个"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unresolved_type_ref`：类型引用无法降级为类型（resolve 未捕获的残余）。
pub fn unresolved_type_ref(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unresolved_type_ref",
        format!("无法解析的类型引用：{name}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unsupported_in_this_phase`：该语法形式在当前类型检查里程碑
/// 尚未覆盖（仅在里程碑之间过渡使用；M8 退出闸门要求零到达）。
pub fn unsupported_in_this_phase(what: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unsupported_in_this_phase",
        format!("当前类型检查阶段暂不支持：{what}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::break_not_in_loop`：`break`/`continue` 出现在循环体外。
pub fn break_not_in_loop(what: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::break_not_in_loop",
        format!("`{what}` 只能出现在循环体内"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::no_applicable_overload`：没有重载候选可接受给定的实参。
pub fn no_applicable_overload(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::no_applicable_overload",
        "没有匹配的重载候选（实参类型 / 数量不匹配）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::ambiguous_overload`：多个重载候选同等匹配，无法选择。
pub fn ambiguous_overload(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::ambiguous_overload",
        "重载解析有歧义：多个候选同等匹配",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::return_type_mismatch`：return 表达式类型与声明返回类型不匹配。
pub fn return_type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::return_type_mismatch",
        format!("返回类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_body_not_supported`：annotation class 不支持类型体。
pub fn annotation_class_body_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_body_not_supported",
        "annotation class 不支持类型体",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_type_params_not_supported`：annotation class 不支持类型参数。
pub fn annotation_class_type_param_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_type_params_not_supported",
        "annotation class 不支持类型参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_effect_param_not_supported`：annotation class 不支持 effect 参数。
pub fn annotation_class_eff_param_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_effect_param_not_supported",
        "annotation class 不支持 effect 参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::builtin_annotation_invalid_target`：内建注解目标不合法。
pub fn builtin_annotation_invalid_target(ann: &str, allowed: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::builtin_annotation_invalid_target",
        format!("内建注解 `{ann}` 只能用于 {allowed}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_arg_missing_required`：注解缺少必填参数。
pub fn annotation_arg_missing_required(ann: &str, param: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_arg_missing_required",
        format!("注解 `{ann}` 缺少必填参数 `{param}`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::experimental_annotation_arg_must_be_string`。
pub fn experimental_annotation_arg_must_be_string(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::experimental_annotation_arg_must_be_string",
        "`@Experimental` 的 `feature` 参数必须是字符串字面量",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::experimental_annotation_invalid_arg_shape`。
pub fn experimental_annotation_invalid_arg_shape(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::experimental_annotation_invalid_arg_shape",
        "`@Experimental` 只支持固定形状 `@Experimental(feature = \"some_feature\")`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::suppress_annotation_requires_warning_codes`。
pub fn suppress_annotation_requires_warning_codes(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::suppress_annotation_requires_warning_codes",
        "`@Suppress` 至少需要一个 warning code",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::suppress_annotation_named_args_not_supported`。
pub fn suppress_annotation_named_args_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::suppress_annotation_named_args_not_supported",
        "`@Suppress` 不支持命名参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::suppress_annotation_arg_must_be_string`。
pub fn suppress_annotation_arg_must_be_string(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::suppress_annotation_arg_must_be_string",
        "`@Suppress` 的参数必须是字符串字面量",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unknown_suppress_warning_code`。
pub fn unknown_suppress_warning_code(code: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unknown_suppress_warning_code",
        format!("未知的 warning code：`{code}`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::duplicate_enum_variant_field`：enum variant 字段重名。
pub fn duplicate_enum_variant_field(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::duplicate_enum_variant_field",
        "enum variant 字段重复定义",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::effectful_function_type_cast_not_supported`。
pub fn effectful_function_type_cast_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::effectful_function_type_cast_not_supported",
        "当前语言 contract 下，带非 `Pure` effect row 的函数类型不能参与显式 `as`/`as?`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::intrinsic_decl_requires_trusted_syslib`：@Intrinsic 只能在受信任 syslib 中声明。
pub fn intrinsic_decl_requires_trusted_syslib(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::intrinsic_decl_requires_trusted_syslib",
        "@Intrinsic 声明只能在受信任的系统库（sysroot）中使用",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::intrinsic_fun_must_have_no_body`：@Intrinsic 函数不能有 body。
pub fn intrinsic_fun_must_have_no_body(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::intrinsic_fun_must_have_no_body",
        "@Intrinsic 函数不能有函数体",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::required_effect_not_declared`：函数体执行了未声明的 effect。
pub fn required_effect_not_declared(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::required_effect_not_declared",
        "函数体执行了 effect，但签名中未声明 effect row",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::static_initializer_must_be_pure`：顶层初始化器必须 Pure。
pub fn static_initializer_must_be_pure(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::static_initializer_must_be_pure",
        "顶层 val 初始化器必须是 Pure",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::binary_op_operand_type_mismatch`：二元运算符操作数类型不匹配。
pub fn binary_op_operand_type_mismatch(op: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::binary_op_operand_type_mismatch",
        format!("运算符 `{op}` 的操作数类型不匹配：{found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::operator_overload_not_found`：找不到运算符重载方法。
pub fn operator_overload_not_found(op: &str, ty: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::operator_overload_not_found",
        format!("类型 {ty} 没有运算符 `{op}` 的重载方法"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::return_not_in_function_body`：`return` 出现在 lambda / init 块中。
pub fn return_not_in_function_body(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::return_not_in_function_body",
        "`return` 只能出现在命名函数体内",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::conflicting_overloads`：签名相同的重载冲突。
pub fn conflicting_overloads(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::conflicting_overloads",
        format!("重载冲突：{name} 存在签名相同的重载"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::binary_op_operand_type_mismatch`：二元运算操作数类型不匹配。
pub fn binary_op_operand_type_mismatch_detail(
    op: &str,
    left: &str,
    right: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::binary_op_operand_type_mismatch",
        format!("运算符 `{op}` 的操作数类型不匹配：{left} 与 {right}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::when_non_exhaustive_missing_variants`：when 不穷尽。
pub fn when_non_exhaustive_missing_variants(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_non_exhaustive_missing_variants",
        "when 表达式不穷尽：缺少 enum variant / else 分支",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::where_constraint_not_satisfied`：where 约束不满足。
pub fn where_constraint_not_satisfied(constraint: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::where_constraint_not_satisfied",
        format!("where 约束不满足：{constraint}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::virtual_method_cannot_be_generic`：虚方法不能有方法级类型参数。
pub fn virtual_method_cannot_be_generic(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::virtual_method_cannot_be_generic",
        "虚方法（open/abstract/override/interface 方法）不能引入方法级类型参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::struct_lit_unknown_field`：struct 字面量中的未知字段。
pub fn struct_lit_unknown_field(field: &str, ty: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::struct_lit_unknown_field",
        format!("类型 {ty} 不存在字段 `{field}`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::struct_lit_field_type_mismatch`：struct 字面量字段类型不匹配。
pub fn struct_lit_field_type_mismatch(
    field: &str,
    expected: &str,
    found: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::struct_lit_field_type_mismatch",
        format!("字段 `{field}` 初始化值类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::struct_lit_duplicate_field`：struct 字面量中的重复字段。
pub fn struct_lit_duplicate_field(field: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::struct_lit_duplicate_field",
        format!("字段重复：`{field}`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::closure_var_capture_not_allowed`：lambda 不能捕获外层 `var`。
pub fn closure_var_capture_not_allowed(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::closure_var_capture_not_allowed",
        format!("lambda 不能捕获可变局部变量 `{name}`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::return_value_required`：函数声明了非 Unit 返回类型但 return 无值。
pub fn return_value_required(expected: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::return_value_required",
        format!("需要返回值：函数返回类型为 {expected}，但 return 无表达式"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::call_arity_mismatch`：调用实参数量与形参不符（调用专用码）。
pub fn call_arity_mismatch(expected: usize, found: usize, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::call_arity_mismatch",
        format!("参数数量不匹配：期望 {expected} 个，但传入 {found} 个"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::call_arg_type_mismatch`：调用实参类型不匹配（调用专用码）。
pub fn call_arg_type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::call_arg_type_mismatch",
        format!("参数类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::initializer_type_mismatch`：val/property 初始化值类型不匹配。
pub fn initializer_type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::initializer_type_mismatch",
        format!("初始化值类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::array_lit_element_type_mismatch`：数组字面量元素类型不匹配。
pub fn array_lit_element_type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::array_lit_element_type_mismatch",
        format!("数组元素类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unknown_call_arg_name`：调用中使用了未知的命名实参。
pub fn unknown_call_arg_name(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unknown_call_arg_name",
        format!("未知的命名实参：{name}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::call_arg_duplicate`：调用中重复使用了同一命名实参。
pub fn call_arg_duplicate(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::call_arg_duplicate",
        format!("重复的命名实参：{name}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_where_clause_not_supported`。
pub fn annotation_class_where_clause_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_where_clause_not_supported",
        "annotation class 不支持 where 子句",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_supertypes_not_supported`。
pub fn annotation_class_supertypes_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_supertypes_not_supported",
        "annotation class 不支持继承",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_param_must_be_val`。
pub fn annotation_class_param_must_be_val(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_param_must_be_val",
        format!("annotation class 参数 `{name}` 必须是 `val`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_must_be_class`。
pub fn annotation_class_must_be_class(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_must_be_class",
        "`annotation` 修饰符必须是 `class`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_modifier_not_supported`：annotation class 不支持指定修饰符。
pub fn annotation_class_modifier_not_supported_detail(mod_name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_modifier_not_supported",
        format!("annotation class 不支持修饰符 `{mod_name}`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_class_modifier_not_supported`（旧接口，不使用）。
pub fn annotation_class_modifier_not_supported(span: Span) -> Diagnostic {
    annotation_class_modifier_not_supported_detail("modifier", span)
}

/// `scoop::typecheck::annotation_class_effect_param_not_supported`。
pub fn annotation_class_effect_param_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_class_effect_param_not_supported",
        "annotation class 不支持 effect 参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_type_is_not_annotation_class`。
pub fn annotation_type_is_not_annotation_class(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_type_is_not_annotation_class",
        format!("`{name}` 不是 annotation class"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_arg_not_const`。
pub fn annotation_arg_not_const(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_arg_not_const",
        "注解实参必须是编译时常量",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_invalid_target`。
pub fn annotation_invalid_target(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_invalid_target",
        "注解目标不合法",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::meta_annotation_invalid_target`：`@Target`/`@Retention` 只能用于 annotation class。
pub fn meta_annotation_invalid_target(ann: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::meta_annotation_invalid_target",
        format!("内建注解 `{ann}` 只能用于 annotation class"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::where_target_not_in_current_decl`。
pub fn where_target_not_in_current_decl(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::where_target_not_in_current_decl",
        "where 约束目标必须是当前声明的类型参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::duplicate_where_constraint`。
pub fn duplicate_where_constraint(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::duplicate_where_constraint",
        "重复的 where 约束",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::call_arg_positional_after_named`。
pub fn call_arg_positional_after_named(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::call_arg_positional_after_named",
        "位置实参不能出现在命名实参之后",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::array_lit_type_annotation_required`。
pub fn array_lit_type_annotation_required(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::array_lit_type_annotation_required",
        "空数组字面量需要类型标注",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::fun_must_have_body`：普通函数必须提供函数体。
pub fn fun_must_have_body(span: Span) -> Diagnostic {
    fun_must_have_body_detail("普通函数必须提供函数体", span)
}

/// `scoop::typecheck::fun_must_have_body`（带上下文详情）。
pub fn fun_must_have_body_detail(what: &str, span: Span) -> Diagnostic {
    Diagnostic::error("scoop::typecheck::fun_must_have_body", what).with_primary(span, "这里")
}

/// `scoop::typecheck::intrinsic_type_field_not_supported`：@Intrinsic 类型不能声明字段。
pub fn intrinsic_type_field_not_supported(field_fqn: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::intrinsic_type_field_not_supported",
        format!("`@Intrinsic` 类型不能声明字段：{field_fqn}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::intrinsic_type_interface_override_must_be_bodied_regular_method`。
pub fn intrinsic_type_interface_override_must_be_bodied_regular_method(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::intrinsic_type_interface_override_must_be_bodied_regular_method",
        "`@Intrinsic` 类型的 interface override 必须是带 body 的普通 method",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::entry_point_main_invalid_signature`：main 函数签名不合法。
pub fn entry_point_main_invalid_signature(detail: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::entry_point_main_invalid_signature",
        format!("entry-point `main` 签名不合法：{detail}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::entry_point_must_be_pure`：程序入口必须 Pure。
pub fn entry_point_must_be_pure(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::entry_point_must_be_pure",
        "程序入口 `main` 的 effect row 必须是 Pure",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::entry_point_must_be_closed_pure`：程序入口 effect row 必须闭合。
pub fn entry_point_must_be_closed_pure(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::entry_point_must_be_closed_pure",
        "程序入口 `main` 必须使用闭合 effect row（`Pure!`）",
    )
    .with_primary(span, "这里")
}

// ===== `with` 更新表达式校验（spec §2.6 / §8.4）=====

/// `scoop::typecheck::with_update_base_not_supported`。
pub fn with_update_base_not_supported(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::with_update_base_not_supported",
        format!("`with` 的 base 必须是可复制更新的值类型（struct/tuple/enum），但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::with_update_duplicate_path`。
pub fn with_update_duplicate_path(path: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::with_update_duplicate_path",
        format!("`with` 更新字段路径重复：{path}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::with_update_overlapping_paths`。
pub fn with_update_overlapping_paths(parent: &str, child: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::with_update_overlapping_paths",
        format!("`with` 更新字段路径冲突：{parent} 与 {child}（并行语义不允许一条路径包含另一条）"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::with_update_unknown_field`。
pub fn with_update_unknown_field(struct_name: &str, field: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::with_update_unknown_field",
        format!("`{struct_name}` 不存在字段：{field}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::with_update_field_type_mismatch`。
pub fn with_update_field_type_mismatch(
    struct_name: &str,
    field: &str,
    expected: &str,
    found: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::with_update_field_type_mismatch",
        format!("`{struct_name}.{field}` 更新值类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::tuple_member_old_syntax`。
pub fn tuple_member_old_syntax(old: &str, new: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::tuple_member_old_syntax",
        format!("tuple 字段索引请写成 `{new}`；旧写法 `{old}` 已移除"),
    )
    .with_primary(span, "这里")
}

// ===== @Extern 函数 ABI 校验（spec §15.x）=====

/// `scoop::typecheck::extern_annotation_abi_not_supported`：暂不支持的 ABI 名。
pub fn extern_annotation_abi_not_supported(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_annotation_abi_not_supported",
        format!("暂不支持的 `@Extern` ABI：{name}（当前仅支持 \"c\" / \"scoop\"）"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_annotation_arg_duplicate`：`@Extern` 命名参数重复指定。
pub fn extern_annotation_arg_duplicate(param: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_annotation_arg_duplicate",
        format!("`@Extern` 参数 `{param}` 重复指定"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_annotation_args_invalid`：`@Extern` 实参形态不合法。
pub fn extern_annotation_args_invalid(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_annotation_args_invalid",
        "`@Extern` 仅支持：无参 / 单个字符串位置参数 / 命名参数 `name`、`lib`，以及函数声明上的 `abi`、`callingConvention`（字符串字面量）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::calling_convention_not_supported`：暂不支持的 calling convention。
pub fn calling_convention_not_supported(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::calling_convention_not_supported",
        format!("暂不支持的 calling convention：{name}（当前仅支持默认 C ABI：\"c\"/\"cdecl\"）"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_calling_convention_annotation_not_allowed`。
pub fn extern_fun_calling_convention_annotation_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_calling_convention_annotation_not_allowed",
        "`@Extern` 函数不再支持单独叠加 `@CallingConvention`；请改用 `@Extern(..., callingConvention = \"...\")`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_c_abi_modifier_redundant`：`abi = "c"` 已隐含该修饰符。
pub fn extern_fun_c_abi_modifier_redundant(annotation: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_c_abi_modifier_redundant",
        format!("`abi = \"c\"` 的 `@Extern` 已隐含 `{annotation}`，不允许重复标注"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_scoop_abi_modifier_not_supported`：scoop ABI 不支持该修饰符。
pub fn extern_fun_scoop_abi_modifier_not_supported(annotation: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_scoop_abi_modifier_not_supported",
        format!("`abi = \"scoop\"` 的 `@Extern` 不支持 `{annotation}`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_scoop_abi_calling_convention_not_supported`。
pub fn extern_fun_scoop_abi_calling_convention_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_scoop_abi_calling_convention_not_supported",
        "`abi = \"scoop\"` 当前不支持 `callingConvention`；Managed ABI 不是 machine calling convention 扩展点",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_eff_param_not_allowed`：`@Extern` 不允许 effect row 参数。
pub fn extern_fun_eff_param_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_eff_param_not_allowed",
        "`@Extern` 函数不允许声明 effect row 参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_effects_not_allowed`：`@Extern` 不允许非 Pure effect row。
pub fn extern_fun_effects_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_effects_not_allowed",
        "`@Extern` 函数不允许声明非 Pure 的 effect row",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_scoop_abi_requires_top_level_fun`。
pub fn extern_fun_scoop_abi_requires_top_level_fun(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_scoop_abi_requires_top_level_fun",
        "`abi = \"scoop\"` 当前只支持无 receiver 的顶层函数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_scoop_abi_generics_not_supported`。
pub fn extern_fun_scoop_abi_generics_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_scoop_abi_generics_not_supported",
        "`abi = \"scoop\"` 当前不支持泛型函数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_scoop_abi_callable_surface_not_supported`。
pub fn extern_fun_scoop_abi_callable_surface_not_supported(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_scoop_abi_callable_surface_not_supported",
        format!("`abi = \"scoop\"` v1 暂不支持 function value / continuation 跨边界：{found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_fun_signature_not_supported_by_native_abi`。
pub fn extern_fun_signature_not_supported_by_native_abi(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_fun_signature_not_supported_by_native_abi",
        format!(
            "`@Extern` 函数的 native ABI 签名只接受当前 native value surface：标量、`UIntPtr`、`Ptr<T>`、纯 `FunPtr<F>` token、tuple，以及 `@CLayout` struct；不接受 {found}；长期 opaque token 请 round-trip `GcHandle.raw: UIntPtr`，短时裸地址借出请使用 `GC.pin/unpin` + `scoop.unsafe.Ptr<T>`"
        ),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extern_var_initializer_not_allowed`：extern 顶层变量不允许 initializer。
pub fn extern_var_initializer_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extern_var_initializer_not_allowed",
        "extern 顶层变量声明必须省略 initializer（外部符号由链接提供）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::top_level_var_requires_threadlocal_or_global`。
pub fn top_level_var_requires_threadlocal_or_global(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::top_level_var_requires_threadlocal_or_global",
        "顶层 `var` 必须显式标注 `@ThreadLocal` 或 `@Global`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::top_level_var_storage_policy_conflict`。
pub fn top_level_var_storage_policy_conflict(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::top_level_var_storage_policy_conflict",
        "不能同时标注 `@ThreadLocal` 与 `@Global`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::top_level_var_type_must_be_gc_free`。
pub fn top_level_var_type_must_be_gc_free(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::top_level_var_type_must_be_gc_free",
        "顶层 `var` 的类型必须是 GC-free 值类型",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::nogc_call_forbidden`：`@NoGC` 上下文禁止调用受管函数。
pub fn nogc_call_forbidden(callee: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::nogc_call_forbidden",
        format!("`@NoGC` 上下文禁止调用非 `@NoGC` / native `@Extern` 函数：{callee}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::calling_convention_fun_generics_not_supported`。
pub fn calling_convention_fun_generics_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::calling_convention_fun_generics_not_supported",
        "`@CallingConvention` 当前不支持泛型函数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::calling_convention_fun_effects_not_allowed`。
pub fn calling_convention_fun_effects_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::calling_convention_fun_effects_not_allowed",
        "`@CallingConvention` 函数不允许声明非 Pure 的 effect row",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::calling_convention_fun_signature_not_supported_by_native_abi`。
pub fn calling_convention_fun_signature_not_supported_by_native_abi(
    found: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::calling_convention_fun_signature_not_supported_by_native_abi",
        format!(
            "`@CallingConvention` 函数的 native ABI 签名只接受当前 native value surface：标量、`UIntPtr`、`Ptr<T>`、tuple、`@CLayout` struct；不接受 {found}"
        ),
    )
    .with_primary(span, "这里")
}

// ===== @ReleaseHook 校验（spec §15.x release hook）=====

/// `scoop::typecheck::release_hook_host_must_be_class`。
pub fn release_hook_host_must_be_class(type_fqn: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_host_must_be_class",
        format!(
            "`@ReleaseHook` 只能用于普通 `class` 宿主（不支持 struct / enum / interface / annotation class）：{type_fqn} 是 {found}"
        ),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_host_must_be_non_generic`。
pub fn release_hook_host_must_be_non_generic(type_fqn: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_host_must_be_non_generic",
        format!("`@ReleaseHook` 宿主必须是 non-generic class：{type_fqn} 声明了类型参数"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_host_must_be_final`。
pub fn release_hook_host_must_be_final(type_fqn: &str, modifier: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_host_must_be_final",
        format!("`@ReleaseHook` 宿主必须是 final class：{type_fqn} 带有 `{modifier}` 修饰符"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_host_requires_experimental`。
pub fn release_hook_host_requires_experimental(type_fqn: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_host_requires_experimental",
        format!(
            "`@ReleaseHook` 宿主 `{type_fqn}` 必须同时标注 `@Experimental(feature = \"releaseHook\")`"
        ),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_function_not_found`。
pub fn release_hook_function_not_found(
    type_fqn: &str,
    function_fqn: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_function_not_found",
        format!("`@ReleaseHook` 宿主 `{type_fqn}` 的释放函数 `{function_fqn}` 不存在"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_function_must_be_nogc_or_c_extern`。
pub fn release_hook_function_must_be_nogc_or_c_extern(
    function_fqn: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_function_must_be_nogc_or_c_extern",
        format!(
            "`@ReleaseHook` 释放函数 `{function_fqn}` 必须是 `@NoGC` 或 `@Extern(abi = \"c\")`"
        ),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_function_return_must_be_unit`。
pub fn release_hook_function_return_must_be_unit(
    function_fqn: &str,
    found: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_function_return_must_be_unit",
        format!("`@ReleaseHook` 释放函数 `{function_fqn}` 的返回类型必须是 Unit，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_arg_count_mismatch`。
pub fn release_hook_arg_count_mismatch(
    type_fqn: &str,
    function_fqn: &str,
    field_count: usize,
    param_count: usize,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_arg_count_mismatch",
        format!(
            "`@ReleaseHook` 宿主 `{type_fqn}` 的 args 数量与释放函数 `{function_fqn}` 参数数量不匹配：args 有 {field_count} 个，参数有 {param_count} 个"
        ),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_arg_field_not_found`。
pub fn release_hook_arg_field_not_found(
    type_fqn: &str,
    field_name: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_arg_field_not_found",
        format!("`@ReleaseHook` 宿主 `{type_fqn}` 没有名为 `{field_name}` 的字段"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_arg_field_must_be_gc_free`。
pub fn release_hook_arg_field_must_be_gc_free(
    type_fqn: &str,
    field_name: &str,
    found: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_arg_field_must_be_gc_free",
        format!(
            "`@ReleaseHook` 字段 `{type_fqn}.{field_name}` 必须是 GC-free 值类型，但得到 {found}"
        ),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::release_hook_arg_type_mismatch`。
pub fn release_hook_arg_type_mismatch(
    type_fqn: &str,
    field_name: &str,
    field_ty: &str,
    param_ty: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::release_hook_arg_type_mismatch",
        format!(
            "`@ReleaseHook` 字段 `{type_fqn}.{field_name}` 类型与释放函数参数不匹配：字段是 {field_ty}，参数是 {param_ty}"
        ),
    )
    .with_primary(span, "这里")
}
