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

/// `scoop::typecheck::break_not_in_loop`：`break` 出现在循环体外。
pub fn break_not_in_loop(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::break_not_in_loop",
        "`break` 只能出现在循环体内",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::continue_not_in_loop`：`continue` 出现在循环体外。
pub fn continue_not_in_loop(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::continue_not_in_loop",
        "`continue` 只能出现在循环体内",
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

/// `scoop::typecheck::duplicate_enum_variant`：enum variant 重名。
pub fn duplicate_enum_variant(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::duplicate_enum_variant",
        "enum variant 重复定义",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::enum_variant_ctor_arity_mismatch`：enum variant 构造实参数量不匹配。
pub fn enum_variant_ctor_arity_mismatch(expected: usize, found: usize, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::enum_variant_ctor_arity_mismatch",
        format!("参数数量不匹配：期望 {expected} 个，但传入 {found} 个"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::duplicate_struct_field`：struct/class 字段重名。
pub fn duplicate_struct_field(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::duplicate_struct_field",
        "struct 字段重复定义",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::struct_field_must_be_val`：struct 字段必须是 val。
pub fn struct_field_must_be_val(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::struct_field_must_be_val",
        "当前语言 contract 下，struct 字段必须是 `val`，不允许 `var`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::struct_lit_not_struct`：struct 字面量用于非 struct 类型。
pub fn struct_lit_not_struct(ty: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::struct_lit_not_struct",
        format!("`{ty}` 必须是 struct 才能使用 struct 字面量语法"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::struct_lit_missing_fields`：struct 字面量缺少必填字段。
pub fn struct_lit_missing_fields(fields: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::struct_lit_missing_fields",
        format!("struct 字面量缺少字段：{fields}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::field_used_without_backing_field`：computed field 引用了 `field`。
pub fn field_used_without_backing_field(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::field_used_without_backing_field",
        "不能引用 `field`：该属性没有 backing field（computed property）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::splice_field_name_not_static`：`value.[expr]` 的字段名必须静态。
pub fn splice_field_name_not_static(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::splice_field_name_not_static",
        "`value.[expr]` 需要编译期已知的字段名（字符串字面量）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unsupported_member_access`：不支持的成员访问。
pub fn unsupported_member_access(member: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unsupported_member_access",
        format!("暂不支持的成员访问：{member}"),
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

/// `scoop::typecheck::for_missing_iterator_method`：iterable 缺少 `iterator()`。
pub fn for_missing_iterator_method(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::for_missing_iterator_method",
        "for 循环的 iterable 必须提供 `iterator()` 方法",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::for_missing_next_method`：iterator 缺少 `next()`。
pub fn for_missing_next_method(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::for_missing_next_method",
        "for 循环的 iterator 必须提供 `next()` 方法",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::for_next_not_option`：next() 必须返回 `Option<T>`。
pub fn for_next_not_option(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::for_next_not_option",
        "for 循环的 `next()` 必须返回 `Option<T>`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::gc_handle_new_requires_ref`：handleNew 参数必须是引用类型。
pub fn gc_handle_new_requires_ref(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::gc_handle_new_requires_ref",
        "`GC.handleNew` 的参数必须是可追踪的引用类型",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::gc_handle_get_requires_handle`：handleGet 参数必须是 GcHandle。
pub fn gc_handle_get_requires_handle(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::gc_handle_get_requires_handle",
        "`GC.handleGet` 的参数必须是 `GcHandle`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::gc_handle_drop_requires_handle`：handleDrop 参数必须是 GcHandle。
pub fn gc_handle_drop_requires_handle(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::gc_handle_drop_requires_handle",
        "`GC.handleDrop` 的参数必须是 `GcHandle`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::intrinsic_decl_requires_trusted_syslib`：@Intrinsic 只能在受信任 syslib 中声明。
pub fn intrinsic_decl_requires_trusted_syslib(kind: &str, name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::intrinsic_decl_requires_trusted_syslib",
        format!("{kind} `{name}` 只能在 trusted `syslib` cone 中声明 `@Intrinsic`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::intrinsic_fun_must_have_no_body`：@Intrinsic 函数不能有 body。
pub fn intrinsic_fun_must_have_no_body(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::intrinsic_fun_must_have_no_body",
        "`@Intrinsic` 函数必须省略函数体",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::required_effect_not_declared`：函数体执行了未声明的 effect。
pub fn required_effect_not_declared(effect: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::required_effect_not_declared",
        format!("缺少效果声明：函数体执行了 {effect} effect，但签名中未声明该 effect row"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::nogc_fun_effects_not_allowed`：`@NoGC` 函数不允许声明非 Pure 的 effect row。
pub fn nogc_fun_effects_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::nogc_fun_effects_not_allowed",
        "`@NoGC` 函数不允许声明非 Pure 的 effect row",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::nogc_fun_eff_param_not_allowed`：`@NoGC` 函数不允许声明 effect row 参数。
pub fn nogc_fun_eff_param_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::nogc_fun_eff_param_not_allowed",
        "`@NoGC` 函数不允许声明 effect row 参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::static_initializer_must_be_pure`：顶层/object 初始化器必须 `Pure!`。
///
/// `what` 为初始化器描述前缀，如 `顶层绑定 \`Broken\``、`object \`pkg.Holder\` init block`、
/// `object \`pkg.Holder\` 属性 \`broken\``。
pub fn static_initializer_must_be_pure(what: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::static_initializer_must_be_pure",
        format!("{what} 初始化器必须为 `Pure!`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::binary_op_operand_type_mismatch`：二元运算符操作数类型不匹配。
pub fn binary_op_operand_type_mismatch(op: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::binary_op_operand_type_mismatch",
        format!("二元运算符 `{op}` 的操作数类型不匹配：{found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::operator_overload_not_found`：找不到运算符重载方法。
pub fn operator_overload_not_found(op: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::operator_overload_not_found",
        format!("操作符 `{op}` 未找到可用的重载"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unary_operator_overload_not_found`：找不到一元运算符重载方法。
pub fn unary_operator_overload_not_found(op: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unary_operator_overload_not_found",
        format!("操作符 `{op}` 未找到可用的重载"),
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

/// `scoop::typecheck::conflicting_overloads`（带 reason / 候选 / 双 label）。
pub fn conflicting_overloads_detail(
    fqn: &str,
    reason: &str,
    candidate_a: &str,
    candidate_b: &str,
    primary_span: Span,
    related_span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::conflicting_overloads",
        format!("重载签名冲突：{fqn}（{reason}）：{candidate_a} <-> {candidate_b}"),
    )
    .with_primary(primary_span, "冲突声明在这里")
    .with_related(related_span, "第一次声明在这里")
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

/// `scoop::typecheck::when_non_exhaustive_missing_variants`（带缺失分支名）。
pub fn when_non_exhaustive_missing_variants_detail(missing: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_non_exhaustive_missing_variants",
        format!("when 表达式不穷尽：缺少 `{missing}` 分支或 else"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::when_missing_else`：非穷尽类型需 else 分支。
pub fn when_missing_else(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_missing_else",
        "when 表达式必须包含 `else` 分支",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::where_constraint_not_satisfied`：where 约束不满足。
pub fn where_constraint_not_satisfied(constraint: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::where_constraint_not_satisfied",
        format!("泛型约束不满足：{constraint}"),
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
        format!("lambda 不能捕获可变局部变量 `{name}`；考虑用 RefCell<T> 共享可变状态、`val snapshot = ...` 获取只读快照，或用 fold / higher-order operators"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::return_value_required`：函数声明了非 Unit 返回类型但 return 无值。
pub fn return_value_required(expected: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::return_value_required",
        format!("缺少返回值：函数返回类型为 {expected}，但 return 无表达式"),
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

/// `scoop::typecheck::assignment_type_mismatch`：赋值类型不匹配。
pub fn assignment_type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::assignment_type_mismatch",
        format!("赋值类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::assignment_target_not_mutable`：赋值目标不可变（`val`）。
pub fn assignment_target_not_mutable(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::assignment_target_not_mutable",
        format!("`{name}` 是 `val`，不可赋值"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::when_guard_not_bool`：when guard 不是 Bool。
pub fn when_guard_not_bool(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_guard_not_bool",
        format!("guard 表达式必须是 Bool 类型，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::while_condition_not_bool`：while 条件不是 Bool。
pub fn while_condition_not_bool(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::while_condition_not_bool",
        format!("条件类型必须是 Bool，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::if_condition_not_bool`：if 条件不是 Bool。
pub fn if_condition_not_bool(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::if_condition_not_bool",
        format!("条件类型必须是 Bool，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::callee_not_callable`：被调用的符号不是可调用函数。
pub fn callee_not_callable(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::callee_not_callable",
        format!("`{name}` 不可调用（不是函数 / 未注册的符号）"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::continuation_not_constructible`：
/// `Continuation` 是 compiler-owned interface，用户代码不能直接构造。
pub fn continuation_not_constructible(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::continuation_not_constructible",
        "`Continuation` 是 compiler-owned interface：只能由编译器/runtime 在 handler 边界物化，用户代码不能直接构造",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::continuation_impl_not_allowed`：
/// 用户代码不能实现/继承 `Continuation`（compiler-owned interface）。
pub fn continuation_impl_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::continuation_impl_not_allowed",
        "`Continuation` 是 compiler-owned interface：用户代码不能实现/继承它",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::continuation_legacy_effect_shorthand_removed`：
/// legacy `Continuation<Resume, eff E>` 简写已移除（必须显式写出 answer type）。
pub fn continuation_legacy_effect_shorthand_removed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::continuation_legacy_effect_shorthand_removed",
        "legacy `Continuation<Resume, eff E>` 简写已移除：必须显式写出 answer type（`Continuation<Resume, Answer, eff E>`）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::continuation_legacy_pure_shorthand_removed`：
/// legacy `Continuation<Resume>` 简写已移除（必须显式写出 answer type）。
pub fn continuation_legacy_pure_shorthand_removed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::continuation_legacy_pure_shorthand_removed",
        "legacy `Continuation<Resume>` 简写已移除：必须显式写出 answer type（`Continuation<Resume, Answer>`）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::closed_effect_row_contains_row_var`：
/// 闭合 effect row（`...!`）不允许引用 effect row 变量（`eff E`）。
pub fn closed_effect_row_contains_row_var(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::closed_effect_row_contains_row_var",
        "闭合 effect row 不允许引用 row 变量（闭合行必须是完全已知的 effect 集合）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::superclass_not_open`：只能继承 `open`/`abstract` 类。
pub fn superclass_not_open(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::superclass_not_open",
        "只能继承 `open` 或 `abstract` 类：超类未声明 `open`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::missing_override`：方法匹配超类的 open 方法，必须声明 `override`。
pub fn missing_override(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::missing_override",
        "方法覆盖了超类的 `open`/`abstract` 方法，必须声明 `override`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::override_non_open_method`：不能覆盖非 open 方法 / 重复签名。
pub fn override_non_open_method(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::override_non_open_method",
        "不能覆盖非 `open` 方法（或以相同签名重复声明）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::override_target_not_found`：`override` 未找到匹配的超类方法。
pub fn override_target_not_found(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::override_target_not_found",
        "`override` 未找到签名匹配的超类方法",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::override_effect_row_not_contained`：
/// 覆盖方法的 effect row 不是超类方法 effect row 的子集（R_over ⊄ R_base）。
pub fn override_effect_row_not_contained(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::override_effect_row_not_contained",
        "覆盖方法的 effect row 不是超类方法 effect row 的子集（effect row 逆变）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::missing_interface_member`：类未实现接口的某个方法。
pub fn missing_interface_member(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::missing_interface_member",
        "类未实现接口要求的成员方法",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_arg_type_mismatch`：注解命名实参类型不匹配。
pub fn annotation_arg_type_mismatch(
    param: &str,
    expected: &str,
    found: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_arg_type_mismatch",
        format!("注解参数 `{param}` 类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::deprecated_annotation_only_first_arg_positional`：
/// `@Deprecated` 只有第一个参数允许位置传递，其余必须命名。
pub fn deprecated_annotation_only_first_arg_positional(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::deprecated_annotation_only_first_arg_positional",
        "`@Deprecated` 只有第一个参数允许使用位置参数，其余参数必须使用命名实参",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::annotation_modifier_invalid_target`：
/// `annotation` 修饰符只能用于 class（annotation class）。
pub fn annotation_modifier_invalid_target(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::annotation_modifier_invalid_target",
        "`annotation` 修饰符只能用于 `annotation class`，不能用于函数/属性",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::vararg_overlaps_non_vararg`：vararg 重载与非 vararg 重载在某 arity 下不可区分。
pub fn vararg_overlaps_non_vararg(
    fqn: &str,
    candidate_a: &str,
    candidate_b: &str,
    primary_span: Span,
    related_span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::vararg_overlaps_non_vararg",
        format!(
            "vararg 与非 vararg 重载重叠（相同 arity 下不可区分）：{fqn}：{candidate_a} <-> {candidate_b}"
        ),
    )
    .with_primary(primary_span, "vararg 声明在这里")
    .with_related(related_span, "第一次声明在这里")
}

/// `scoop::typecheck::vararg_spread_requires_array_or_tuple`：
/// spread 实参 `*expr` 的类型必须是 Array 或 tuple。
pub fn vararg_spread_requires_array_or_tuple(span: Span, suggest_toarray: bool) -> Diagnostic {
    let msg = if suggest_toarray {
        "spread 实参类型不支持：仅接受 Array / tuple，请使用 `.toArray()` 桥接"
    } else {
        "spread 实参类型不支持：仅接受 Array / tuple"
    };
    Diagnostic::error(
        "scoop::typecheck::vararg_spread_requires_array_or_tuple",
        msg,
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::generic_overload_shape_mismatch`：
/// 两个泛型重载的类型参数 shape 不同（仅 differ-by-bound 受支持）。
pub fn generic_overload_shape_mismatch(primary_span: Span, related_span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::generic_overload_shape_mismatch",
        "泛型重载的类型参数 shape 不同：only differ-by-bound generic overloads are supported",
    )
    .with_primary(primary_span, "冲突声明在这里")
    .with_related(related_span, "第一次声明在这里")
}

/// `scoop::typecheck::secondary_ctor_delegation_required`：
/// 有主构造器的 class 中，次构造器必须显式 `: this(...)` 委托。
pub fn secondary_ctor_delegation_required(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::secondary_ctor_delegation_required",
        "class 有主构造器时，次构造器必须写 `: this(...)` 显式委托",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::secondary_ctor_delegation_must_be_this`：
/// 有主构造器时，次构造器只能委托到 `this(...)`，不能 `super(...)`。
pub fn secondary_ctor_delegation_must_be_this(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::secondary_ctor_delegation_must_be_this",
        "class 有主构造器时，次构造器只能委托到 `this(...)`，不能 `super(...)`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::call_receiver_type_mismatch`：receiver 函数调用的 receiver 类型不匹配。
pub fn call_receiver_type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::call_receiver_type_mismatch",
        format!("receiver 类型不匹配：期望 receiver 为 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::call_arity_mismatch`（带函数名）。
pub fn call_arity_mismatch_detail(
    name: &str,
    expected: usize,
    found: usize,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::call_arity_mismatch",
        format!("调用参数数量不匹配：{name} 期望 {expected} 个，但提供了 {found} 个"),
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
        format!("初始化表达式类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::array_lit_element_type_mismatch`：数组字面量元素类型不匹配。
pub fn array_lit_element_type_mismatch(
    index: usize,
    expected: &str,
    found: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::array_lit_element_type_mismatch",
        format!("第 {index} 个元素期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::when_string_pat_not_string`：String literal pattern 只能用于 String 类型。
pub fn when_string_pat_not_string(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_string_pat_not_string",
        "String literal pattern 只能用于 String 类型的 when subject",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::when_variant_pat_not_enum`：variant pattern 只能用于 enum 类型。
pub fn when_variant_pat_not_enum(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_variant_pat_not_enum",
        "variant pattern 只能用于 enum 类型的 when subject",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::when_tuple_pat_not_tuple`：tuple pattern 只能用于 tuple 类型。
pub fn when_tuple_pat_not_tuple(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_tuple_pat_not_tuple",
        "tuple pattern 只能用于 tuple 类型的 when subject",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::when_tuple_pat_too_short`：tuple pattern 带 rest 时前缀超过 tuple 长度。
pub fn when_tuple_pat_too_short(needed: usize, actual: usize, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_tuple_pat_too_short",
        format!("tuple pattern 需要至少 {needed} 个元素，但 tuple 只有 {actual} 个"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::when_variant_pat_too_short`：variant pattern 带 rest 时前缀超过 payload 字段数。
pub fn when_variant_pat_too_short(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::when_variant_pat_too_short",
        "variant pattern 参数不足：带 `..` 时前缀超过 variant payload 字段数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::with_update_unknown_variant`：enum `with` 首段不是已知 variant。
pub fn with_update_unknown_variant(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::with_update_unknown_variant",
        "enum `with` 更新的首段不是该 enum 的 variant（不存在 variant）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::with_update_variant_field_required`：
/// enum `with` 选择 variant 后必须继续给出 payload 字段路径。
pub fn with_update_variant_field_required(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::with_update_variant_field_required",
        "enum `with` 选择 variant 后必须继续给出 payload 字段路径",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::val_variant_pat_not_enum`：val 解构的 variant pattern initializer 非 enum/Option。
pub fn val_variant_pat_not_enum(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::val_variant_pat_not_enum",
        "variant pattern 只能用于 enum / Option 类型的 initializer",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::val_variant_pat_unknown_variant`：val 解构 variant pattern 的 variant 不存在。
pub fn val_variant_pat_unknown_variant(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::val_variant_pat_unknown_variant",
        "val 解构 variant pattern 未找到匹配的 variant",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::val_variant_pat_enum_mismatch`：val 解构 variant pattern 的 enum 前缀不匹配。
pub fn val_variant_pat_enum_mismatch(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::val_variant_pat_enum_mismatch",
        "val 解构 variant pattern 的 enum 前缀不匹配 initializer 类型",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::val_tuple_pat_not_tuple`：val 解构 tuple pattern 的 initializer 非 tuple。
pub fn val_tuple_pat_not_tuple(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::val_tuple_pat_not_tuple",
        "tuple pattern 只能用于 tuple/Unit 类型的 initializer",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::value_only_enum_underlying_not_integral`：value enum 底层类型必须整型。
pub fn value_only_enum_underlying_not_integral(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::value_only_enum_underlying_not_integral",
        "value enum 的底层类型必须是整型标量",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::clayout_struct_must_be_gc_free`：@CLayout struct 字段必须 GC-free。
pub fn clayout_struct_must_be_gc_free(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::clayout_struct_must_be_gc_free",
        "`@CLayout` struct 的所有字段必须是 GC-free 值类型",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::clayout_packed_value_not_supported`：@CLayout(packed) 必须是 2 的幂且在范围内。
pub fn clayout_packed_value_not_supported(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::clayout_packed_value_not_supported",
        "`@CLayout(packed)` 的值必须是 2 的幂（1/2/4/8/16）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::object_not_constructible`：object 是单例，不能构造。
pub fn object_not_constructible(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::object_not_constructible",
        "`object` 是单例，不能作为构造器调用",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::cyclic_type_alias`：循环的类型别名。
pub fn cyclic_type_alias(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::cyclic_type_alias",
        "循环的类型别名：typealias 直接或间接引用自身",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::extension_property_initializer_not_allowed`：扩展属性不允许 initializer。
pub fn extension_property_initializer_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::extension_property_initializer_not_allowed",
        "扩展属性不允许 initializer（应为计算属性或带 accessor）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::call_missing_required_args`：命名实参跳过了无默认值的必需参数。
pub fn call_missing_required_args(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::call_missing_required_args",
        "缺少必需参数：命名实参只能跳过带默认值的形参",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unknown_call_arg_name`：调用中使用了未知的命名实参。
pub fn unknown_call_arg_name(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unknown_call_arg_name",
        format!("没有名为 `{name}` 的参数"),
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
        format!("`{name}` 不是注解类"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unresolved_annotation_type`：注解类型无法解析。
pub fn unresolved_annotation_type(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unresolved_annotation_type",
        format!("未解析的注解类型：{name}"),
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

/// `scoop::typecheck::ref_value_bound_mutually_exclusive`：同一类型参数同时带 `ref` 与 `value` 约束。
pub fn ref_value_bound_mutually_exclusive(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::ref_value_bound_mutually_exclusive",
        "`ref` 与 `value` 约束互斥，不能同时施加于同一类型参数",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::conflicting_where_constraints`：冲突的 class bound。
pub fn conflicting_where_constraints(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::conflicting_where_constraints",
        "where 约束冲突：同一类型参数不能约束到多个 class",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::delegated_property_not_allowed_in_value_type`：值类型不允许委托属性。
pub fn delegated_property_not_allowed_in_value_type(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::delegated_property_not_allowed_in_value_type",
        "值类型（struct / enum）不允许委托属性（`by`）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::delegated_property_missing_get_value`。
pub fn delegated_property_missing_get_value(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::delegated_property_missing_get_value",
        "委托属性缺少 `getValue`：delegate 必须提供 `getValue(thisRef, property)`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::delegated_property_missing_set_value`。
pub fn delegated_property_missing_set_value(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::delegated_property_missing_set_value",
        "可变委托属性缺少 `setValue`：delegate 必须提供 `setValue(thisRef, property, value)`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::delegated_property_get_value_signature_mismatch`。
pub fn delegated_property_get_value_signature_mismatch(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::delegated_property_get_value_signature_mismatch",
        format!("`getValue` 的 `property` 参数必须是 `PropertyMeta`，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::delegated_property_set_value_signature_mismatch`。
pub fn delegated_property_set_value_signature_mismatch(
    property_type_fqn: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::delegated_property_set_value_signature_mismatch",
        format!(
            "委托属性的 delegate 未找到匹配的 `setValue` 签名（期望 setValue(thisRef: .., property: PropertyMeta, value: {property_type_fqn}): Unit）"
        ),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::call_arg_positional_after_named`。
pub fn call_arg_positional_after_named(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::call_arg_positional_after_named",
        "位置实参不能出现在命名参数之后",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::array_lit_type_annotation_required`。
pub fn array_lit_type_annotation_required(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::array_lit_type_annotation_required",
        "空数组字面量需要显式类型标注",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::missing_type_annotation`：顶层 val/var 缺少类型注解。
pub fn missing_type_annotation(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::missing_type_annotation",
        "顶层 `val`/`var` 声明缺少类型注解",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::invalid_annotation_target_name`：@Target 中非法的 target 名。
pub fn invalid_annotation_target_name(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::invalid_annotation_target_name",
        format!("非法的 `AnnotationTarget` 名称：`{name}`"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::generic_type_arg_not_inferred`：泛型函数值使用无法推断类型实参。
pub fn generic_type_arg_not_inferred(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::generic_type_arg_not_inferred",
        "泛型函数作为值使用时无法推断类型实参，需要显式类型标注",
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

/// `scoop::typecheck::not_null_assert_operand_not_nullable`：`!!` 的操作数必须是 nullable。
pub fn not_null_assert_operand_not_nullable(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::not_null_assert_operand_not_nullable",
        "`!!` 的操作数必须是 nullable（Option）类型",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::invalid_cast`：不允许的显式类型转换（如 value ↔ ref）。
pub fn invalid_cast(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::invalid_cast",
        "不允许的显式类型转换（当前阶段不做 value ↔ ref 转换）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unsupported_expr`：当前阶段不支持的表达式形式。
pub fn unsupported_expr(what: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unsupported_expr",
        format!("不支持的表达式形式：{what}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::interpolation_expr_not_to_string`：f-string 插值表达式必须实现 ToString。
pub fn interpolation_expr_not_to_string(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::interpolation_expr_not_to_string",
        "interpolation expr must be ToString（f-string 插值表达式必须实现 ToString）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::value_type_property_must_be_val`：值类型（enum）属性不允许 `var`。
pub fn value_type_property_must_be_val(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::value_type_property_must_be_val",
        "值类型（struct/enum）属性不允许 `var`",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::val_property_setter_not_allowed`：`val` 属性不允许自定义 setter。
pub fn val_property_setter_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::val_property_setter_not_allowed",
        "`val` 属性不允许自定义 setter",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::value_type_property_initializer_not_allowed`：
/// computed 属性（带 getter）不允许 initializer。
pub fn value_type_property_initializer_not_allowed(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::value_type_property_initializer_not_allowed",
        "computed 属性不允许 initializer（带 getter 的计算属性不能同时有初始值）",
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
