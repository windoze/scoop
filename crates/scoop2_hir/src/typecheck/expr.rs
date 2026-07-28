//! 表达式 / 语句类型检查器（M1：单态标量核心）。
//!
//! 当前里程碑（M1）完整覆盖：整型 / 浮点 / Char / String / Bool / Unit 字面量与
//! 默认类型、局部 `val`(带类型标注或推断)、参数、标量二元运算（算术 / 移位 / 位 /
//! 比较 / 逻辑）、一元运算（`!` / `-` / `~`）、`if` / `while` / `return` /
//! `break` / `continue` / 块、赋值（局部 `var`）。
//!
//! **其余形式**（调用、成员访问、`when`、`handle`、lambda、struct/array/tuple
//! 字面量、`is`/`as`、`with`、for、解构等）属于后续里程碑；本阶段遇到时发出
//! `scoop::typecheck::unsupported_in_this_phase`（真实诊断，非占位），M8 退出闸门
//! 要求零到达。
//!
//! 赋值性（M1）：类型相等或 `found` 为 `Nothing`（bottom）。子类型 / 继承 / 强制
//! 转换在后续里程碑补齐。

use std::collections::{HashMap, HashSet};

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{NodeId, Span, Symbol};

use crate::resolve::imports::ImportTable;
use crate::resolve::output::{Resolution, ResolvedValue};
use crate::syntax::ast::{
    self, AssignTargetKind, BinaryOp, Block, CallArg, CastOp, Expr, ExprKind, FunBody, MemberName,
    Param, Stmt, StmtKind, StructLitField, TypeRef, TypeRefKind, UnaryOp, ValBinding,
};
use crate::ty::{FunctionType, NominalType, TypeId, TypeKind, TypeParamType};

use super::diagnostics;
use super::env::{Signature, TypeEnv};
use super::lower::TypeLowering;

/// variant pattern 引用（path + 可选 args），用于 enum 穷尽性按 variant 分组。
struct VariantPatRef<'p> {
    path: &'p crate::syntax::ast::TypePath,
    args: &'p Option<Vec<ast::Pattern>>,
}

/// handler arm 的 op 是否是 `scoop.core.Raise.raise`（即 try/catch 脱糖的 catch arm）。
fn is_raise_raise_op(op: &ast::HandleOp, interner: &scoop2_base::Interner) -> bool {
    interner.resolve(op.op.symbol) == "raise"
        && op.effect_path.segments.len() == 3
        && op
            .effect_path
            .segments
            .iter()
            .zip(["scoop", "core", "Raise"])
            .all(|(seg, expect)| interner.resolve(seg.symbol) == expect)
}

/// handler arm 的 effect operation 去重键：`<effect-path>.<op-name><binder-type-sig>`。
/// 用于检测重复 / 不可达 arm（同一 operation 被多个 arm 覆盖）。
/// 含 binder 类型签名与 op 类型实参，使 `Pair.ping(p: (String,Int))` 与
/// `Pair.ping(p: (Int,String))` 视为不同重载（不误报）。
fn handle_arm_op_key(op: &ast::HandleOp, interner: &scoop2_base::Interner) -> Option<String> {
    let path: String = op
        .effect_path
        .segments
        .iter()
        .map(|s| interner.resolve(s.symbol))
        .collect::<Vec<_>>()
        .join(".");
    let op_name = interner.resolve(op.op.symbol);
    if path.is_empty() {
        return None;
    }
    let mut key = format!("{path}.{op_name}");
    // effect 路径上的类型实参（`Pair<String, Int>.ping`）。
    if !op.effect_args.is_empty() {
        key.push_str("<<");
        key.push_str(
            &op.effect_args
                .iter()
                .map(|a| type_arg_text(a, interner))
                .collect::<Vec<_>>()
                .join(", "),
        );
        key.push_str(">>");
    }
    // op 类型实参（`Query.ask<Int>`）。
    if !op.op_type_args.is_empty() {
        key.push('<');
        key.push_str(
            &op.op_type_args
                .iter()
                .map(|a| type_arg_text(a, interner))
                .collect::<Vec<_>>()
                .join(", "),
        );
        key.push('>');
    }
    // binder 类型签名（`p: (String, Int)`）：把每个带类型的 binder 的类型文本拼入键。
    if !op.binders.is_empty() {
        key.push('(');
        let binders: Vec<String> = op
            .binders
            .iter()
            .map(|b| match &b.ty {
                Some(t) => format!("{}: {}", interner.resolve(b.name.symbol), type_ref_text(t)),
                None => interner.resolve(b.name.symbol).to_string(),
            })
            .collect();
        key.push_str(&binders.join(", "));
        key.push(')');
    }
    Some(key)
}

/// 类型引用的稳定文本（用于去重键；不追求精确，只要同构 TypeRef 产出相同串）。
/// 用 span 偏移作为段名替身（同一文件内唯一），避免借用 interner。
fn type_ref_text(t: &ast::TypeRef) -> String {
    use crate::syntax::ast::TypeRefKind;
    match &t.kind {
        TypeRefKind::Path { path, args } => {
            let mut s: String = path
                .segments
                .iter()
                .map(|seg| format!("seg{}", seg.span.start))
                .collect::<Vec<_>>()
                .join(".");
            if !args.is_empty() {
                s.push('<');
                s.push_str(
                    &args
                        .iter()
                        .map(type_arg_text_simple)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                s.push('>');
            }
            s
        }
        TypeRefKind::Unit => "()".to_string(),
        TypeRefKind::Tuple(els) => {
            let inner: Vec<String> = els.iter().map(type_ref_text).collect();
            format!("({})", inner.join(", "))
        }
        TypeRefKind::Function { params, ret, .. } => {
            let p: Vec<String> = params.iter().map(type_ref_text).collect();
            format!("({}) -> {}", p.join(", "), type_ref_text(ret))
        }
        TypeRefKind::ReceiverFunction {
            receiver,
            params,
            ret,
            ..
        } => {
            let p: Vec<String> = params.iter().map(type_ref_text).collect();
            format!(
                "{}.({}) -> {}",
                type_ref_text(receiver),
                p.join(", "),
                type_ref_text(ret)
            )
        }
        TypeRefKind::Nullable(inner) => format!("{}?", type_ref_text(inner)),
    }
}

fn type_arg_text(arg: &ast::TypeArg, _interner: &scoop2_base::Interner) -> String {
    type_arg_text_simple(arg)
}

fn type_arg_text_simple(arg: &ast::TypeArg) -> String {
    use crate::syntax::ast::TypeArgKind;
    match &arg.kind {
        TypeArgKind::Type(t) => type_ref_text(t),
        TypeArgKind::Star => "*".to_string(),
        TypeArgKind::Effect(_) => "eff".to_string(),
    }
}

/// 判断初始化表达式是否为「无歧义字面量」（类型确定、无重载/推断歧义）。
/// 这类初始化器与声明类型不匹配是真实错误，即便在 lenient 模式下也应报。
/// 覆盖：标量字面量、元组字面量、数组字面量、`()Unit`、`true/false`、嵌套这些。
fn init_expr_is_unambiguous_literal(kind: &ast::ExprKind) -> bool {
    match kind {
        ast::ExprKind::IntLit(_)
        | ast::ExprKind::FloatLit(_)
        | ast::ExprKind::CharLit(_)
        | ast::ExprKind::StringLit(_)
        | ast::ExprKind::UnitLit => true,
        // `true` / `false` 是 Ident（标量字面量）。
        ast::ExprKind::Ident(_) => true,
        ast::ExprKind::TupleLit(els) => els
            .iter()
            .all(|e| init_expr_is_unambiguous_literal(&e.kind)),
        ast::ExprKind::ArrayLit(els) => els
            .iter()
            .all(|e| init_expr_is_unambiguous_literal(&e.kind)),
        // `do { expr; }` 的类型确定（block 体无歧义），应参与类型检查。
        ast::ExprKind::DoBlock(_) | ast::ExprKind::UnsafeBlock(_) | ast::ExprKind::SafeBlock(_) => {
            true
        }
        // 结构体字面量 `Point { x: 1 }` 类型确定（结构体名明确），应参与类型检查。
        ast::ExprKind::StructLit { .. } => true,
        _ => false,
    }
}

/// 检查一个函数体（M1+）。
#[allow(clippy::too_many_arguments)]
pub fn check_function<'a, 'i>(
    params: &[Param],
    return_ty: Option<&TypeRef>,
    body: &FunBody,
    declared_effect: Option<&ast::EffectRowExpr>,
    env: &mut TypeEnv<'i>,
    imports: &'a ImportTable,
    resolution: &'a Resolution,
    diags: &'a mut DiagnosticSink,
    package_prefix: &str,
    type_params: HashMap<Symbol, TypeParamType>,
    param_ref_bounds: HashSet<Symbol>,
    param_value_bounds: HashSet<Symbol>,
    this_ty: Option<TypeId>,
    skip_effect_check: bool,
    expr_types: &'a mut crate::resolve::output::NodeIdTable<TypeId>,
    is_entry_main: bool,
    in_nogc: bool,
) {
    let mut c = ExprChecker {
        env,
        imports,
        resolution,
        diags,
        package_prefix: package_prefix.to_string(),
        type_params,
        param_ref_bounds,
        param_value_bounds,
        locals: HashMap::new(),
        local_vars: HashMap::new(),
        lambda_depth: 0,
        return_ty: None,
        in_loop: 0u32,
        this_ty,
        in_function_body: true,
        in_nogc,
        lenient_type_errors: false,
        performed_effects: Vec::new(),
        escape_effects: Vec::new(),
        effect_suspend_depth: 0,
        expr_types,
    };
    for p in params {
        let ty = match &p.ty {
            Some(t) => c.lower_type(t),
            None => c.env.store.nothing(),
        };
        c.locals.insert(p.name.symbol, ty);
    }
    let ret = return_ty.map(|t| c.lower_type(t));
    c.return_ty = ret;
    #[allow(unused_assignments)]
    let mut body_value_ty = None;
    match body {
        FunBody::Block(b) => {
            body_value_ty = Some(c.walk_block(b));
        }
        FunBody::Expr(e) => {
            let t = c.walk_expr(e);
            body_value_ty = Some(t);
            // 块表达式体（`fun f(): T = { ... return x }`）：return 类型由 return 语句检查，
            // 不在此检查块值（块值可能为 Unit 因为最后一条是 return 而非 expr）。
            let is_block_body = matches!(
                e.kind,
                ExprKind::Block(_)
                    | ExprKind::DoBlock(_)
                    | ExprKind::UnsafeBlock(_)
                    | ExprKind::SafeBlock(_)
            );
            if !is_block_body
                && let Some(expected) = c.return_ty
                && !c.assignable(t, expected)
            {
                c.diags.push(diagnostics::type_mismatch(
                    &c.describe(expected),
                    &c.describe(t),
                    e.span,
                ));
            }
        }
    }
    // entry-point main：未显式标注返回类型时，按推断的 body 值类型校验（必须 Unit 或 Int）。
    if is_entry_main
        && ret.is_none()
        && let Some(bvt) = body_value_ty
        && !c.env.store.is_unit(bvt)
        && !matches!(
            c.env.store.kind(bvt),
            TypeKind::Value(crate::ty::ValueTypeKind::Int)
        )
        && !c.env.store.is_nothing(bvt)
    {
        let desc = c.describe(bvt);
        c.diags
            .push(diagnostics::entry_point_main_invalid_signature(
                &format!("返回类型为 `{desc}`，必须是 Unit 或 Int"),
                return_ty.as_ref().map(|t| t.span).unwrap_or_default(),
            ));
    }
    // effect 检查：函数体执行的 effect 必须在声明的 effect row 中。
    // private/internal 函数省略 effect row 时跳过（由函数体推断）。
    if !skip_effect_check {
        let declared = ExprChecker::extract_effect_names(declared_effect, c.env.interner);
        // 若已存在结构性 handle 诊断（如 handle_arm_unreachable），不额外报 required_effect_not_declared
        // —— 结构性诊断优先（避免掩盖 handle arm 不可达等更精确的诊断）。
        let has_structural_handle_diag = c
            .diags
            .iter()
            .any(|d| d.code == "scoop::typecheck::handle_arm_unreachable");
        for (performed, span) in &c.performed_effects {
            if !declared.contains(performed) {
                c.diags
                    .push(diagnostics::required_effect_not_declared(performed, *span));
                break;
            }
        }
        if !has_structural_handle_diag {
            for (performed, span) in &c.escape_effects {
                if !declared.contains(performed) {
                    c.diags
                        .push(diagnostics::required_effect_not_declared(performed, *span));
                    break;
                }
            }
        }
    }
}
pub fn check_top_level_val<'a, 'i>(
    val: &ast::ValDecl,
    env: &mut TypeEnv<'i>,
    imports: &'a ImportTable,
    resolution: &'a Resolution,
    diags: &'a mut DiagnosticSink,
    package_prefix: &str,
    expr_types: &'a mut crate::resolve::output::NodeIdTable<TypeId>,
) {
    let mut c = ExprChecker {
        env,
        imports,
        resolution,
        diags,
        package_prefix: package_prefix.to_string(),
        type_params: HashMap::new(),
        param_ref_bounds: HashSet::new(),
        param_value_bounds: HashSet::new(),
        locals: HashMap::new(),
        local_vars: HashMap::new(),
        lambda_depth: 0,
        return_ty: None,
        in_loop: 0u32,
        this_ty: None,
        in_function_body: false,
        in_nogc: false,
        lenient_type_errors: true,
        performed_effects: Vec::new(),
        escape_effects: Vec::new(),
        effect_suspend_depth: 0,
        expr_types,
    };
    let declared = val.ty.as_ref().map(|t| c.lower_type(t));
    let init_ty = val.init.as_ref().map(|e| c.walk_expr(e));
    // 空数组字面量 `[]` 无显式类型标注时必须报错（无法推断元素类型）。
    if declared.is_none()
        && val
            .init
            .as_ref()
            .is_some_and(|e| matches!(&e.kind, ExprKind::ArrayLit(els) if els.is_empty()))
    {
        c.diags
            .push(diagnostics::array_lit_type_annotation_required(
                val.init.as_ref().map(|e| e.span).unwrap_or_default(),
            ));
    }
    // lenient 模式抑制类型级假阳性（重载决议未定等），但**字面量初始化器**类型确定、
    // 无重载歧义，其与声明类型不匹配是真实错误，必须报（即使 lenient）。
    let init_is_literal = val
        .init
        .as_ref()
        .is_some_and(|e| init_expr_is_unambiguous_literal(&e.kind));
    // 星投影类型 `Array<*>` 作为声明类型：显式值赋值是确定的真实错误（非重载假阳性），
    // 即使 lenient 也报。
    let declared_is_star = declared
        .is_some_and(|d| matches!(c.env.store.kind(d), TypeKind::StarProjection))
        || declared.is_some_and(|d| {
            // nominal 的类型实参含 StarProjection 也算（`Array<*>`）。
            match c.env.store.kind(d) {
                TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
                | TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => n
                    .args
                    .iter()
                    .any(|&a| matches!(c.env.store.kind(a), TypeKind::StarProjection)),
                _ => false,
            }
        });
    if let (Some(d), Some(i)) = (declared, init_ty)
        && !c.env.store.is_unit(d)
        && !c.assignable(i, d)
        && (!c.lenient_type_errors || init_is_literal || declared_is_star)
        && !matches!(
            c.env.store.kind(i),
            TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
        )
        && !c.is_array_lit_to_mutable_array(val.init.as_ref(), d, i)
        && !c.is_empty_array_lit_to_array(val.init.as_ref(), d, i)
    {
        c.diags.push(diagnostics::initializer_type_mismatch(
            &c.describe(d),
            &c.describe(i),
            val.init
                .as_ref()
                .map(|e| e.span)
                .unwrap_or_else(|| val.ty.as_ref().map(|t| t.span).unwrap_or_default()),
        ));
    }
    // 顶层 val 初始化器必须为 Pure!：执行了任何 effect 即报错。
    let eff_span = c.performed_effects.first().map(|(_, s)| *s);
    if let Some(span) = eff_span {
        let name = match &val.binding {
            ast::ValBinding::Name(id) => c.env.interner.resolve(id.symbol).to_string(),
            ast::ValBinding::Pattern(_) => "<pattern>".to_string(),
        };
        let what = format!("顶层绑定 `{name}`");
        c.diags
            .push(diagnostics::static_initializer_must_be_pure(&what, span));
    }
}

/// 静态初始化器遍历目标：object `init` 块（Block）或属性初始化表达式（Expr）。
pub(super) enum PureInitSite<'s> {
    Block(&'s ast::Block),
    Expr(&'s ast::Expr),
}

/// 静态初始化器（object `init` 块 / object 属性初始化器）必须 `Pure!`。
///
/// 用 lenient ExprChecker 遍历初始化表达式，收集 performed effects；非空则报错。
/// `what` 为初始化器描述前缀（如 `object \`pkg.Holder\` init block`）。
#[allow(clippy::too_many_arguments)]
pub(super) fn check_pure_static_init<'a, 'i>(
    env: &mut TypeEnv<'i>,
    imports: &'a ImportTable,
    resolution: &'a Resolution,
    diags: &'a mut DiagnosticSink,
    package_prefix: &str,
    this_ty: Option<TypeId>,
    what: String,
    site: PureInitSite<'_>,
    expr_types: &'a mut crate::resolve::output::NodeIdTable<TypeId>,
) {
    let mut c = ExprChecker {
        env,
        imports,
        resolution,
        diags,
        package_prefix: package_prefix.to_string(),
        type_params: HashMap::new(),
        param_ref_bounds: HashSet::new(),
        param_value_bounds: HashSet::new(),
        locals: HashMap::new(),
        local_vars: HashMap::new(),
        lambda_depth: 0,
        return_ty: None,
        in_loop: 0u32,
        this_ty,
        in_function_body: false,
        in_nogc: false,
        lenient_type_errors: true,
        performed_effects: Vec::new(),
        escape_effects: Vec::new(),
        effect_suspend_depth: 0,
        expr_types,
    };
    match site {
        PureInitSite::Block(b) => {
            c.walk_block(b);
        }
        PureInitSite::Expr(e) => {
            c.walk_expr(e);
        }
    }
    if let Some(span) = c.performed_effects.first().map(|(_, s)| *s) {
        c.diags
            .push(diagnostics::static_initializer_must_be_pure(&what, span));
    }
}

struct ExprChecker<'a, 'i> {
    env: &'a mut TypeEnv<'i>,
    imports: &'a ImportTable,
    resolution: &'a Resolution,
    diags: &'a mut DiagnosticSink,
    package_prefix: String,
    type_params: HashMap<Symbol, TypeParamType>,
    /// 类型参数名 → 是否声明了 `ref` bound（ref/value kind bound 检查用）。
    param_ref_bounds: HashSet<Symbol>,
    /// 类型参数名 → 是否声明了 `value` bound。
    param_value_bounds: HashSet<Symbol>,
    /// 局部名 → 类型（参数 + 局部 `val`）。M1 用扁平表（遮蔽语义简化）。
    locals: HashMap<Symbol, TypeId>,
    /// 局部名 → var 声明所在的 lambda 深度（检测 lambda 捕获可变变量）。
    local_vars: HashMap<Symbol, u32>,
    /// 当前嵌套 lambda 深度（0 = 命名函数体）。用于检测 var 捕获。
    lambda_depth: u32,
    return_ty: Option<TypeId>,
    in_loop: u32,
    /// 成员函数体的 `this` 类型（lowered nominal）；非成员函数为 `None`。
    this_ty: Option<TypeId>,
    /// 当前是否在命名函数体内（非 lambda / 非 init 块）。`return` 只允许在此上下文。
    in_function_body: bool,
    /// 当前函数是否标注 `@NoGC`（体内禁止调用非 `@NoGC` / native `@Extern(c)` 函数）。
    in_nogc: bool,
    /// 顶层 val init 检查：lenient 模式，抑制类型级假阳性（arity / type mismatch），
    /// 但保留结构性检查（when 穷尽 / 命名实参形态 / 泛型推断等）。
    lenient_type_errors: bool,
    /// 当前函数体执行了的 effect（FQN, span）（effect 检查用）。
    performed_effects: Vec<(String, Span)>,
    /// 穿透 effect（resume 的 resumed-step effect）：不受 handle arm 体截断影响，
    /// 最终合并到 performed_effects 参与函数级 effect 检查。
    escape_effects: Vec<(String, Span)>,
    /// effect 采集挂起深度（> 0 时不在 performed_effects 中记录；lambda / handle 体用）。
    effect_suspend_depth: u32,
    /// per-NodeId 表达式推断类型写回表（typed-HIR / dump-hir 消费）。
    /// 调用方提供，`walk_expr` 包装层在每次返回前写入 `expr.id → ty`。
    expr_types: &'a mut crate::resolve::output::NodeIdTable<TypeId>,
}

impl<'a, 'i> ExprChecker<'a, 'i> {
    fn lower_type(&mut self, ty: &TypeRef) -> TypeId {
        let mut lower = TypeLowering::with_bounds(
            self.env,
            self.imports,
            self.type_params.clone(),
            self.package_prefix.clone(),
            self.diags,
            self.param_ref_bounds.clone(),
            self.param_value_bounds.clone(),
        );
        lower.lower(ty)
    }

    fn describe(&self, id: TypeId) -> String {
        // 委托给统一的类型渲染器（短名；nominal 用末段名）。
        crate::ty::render_type(&self.env.store, self.env.interner, id, false)
    }

    /// 赋值性（M1+）：相等、`Nothing` bottom、nominal 子类型（继承/接口）、
    /// 函数逆变/协变、Option/元组协变。
    fn assignable(&self, found: TypeId, expected: TypeId) -> bool {
        self.assignable_with(found, expected, true)
    }

    /// 严格子类型（不含整型/浮点字面量吸收），用于重载特殊性比较。
    fn assignable_strict(&self, found: TypeId, expected: TypeId) -> bool {
        self.assignable_with(found, expected, false)
    }

    fn assignable_with(&self, found: TypeId, expected: TypeId, absorb: bool) -> bool {
        if found == expected || self.env.store.is_nothing(found) {
            return true;
        }
        if absorb {
            // 整数字面量吸收：Int 可赋值给任何整数类型（Int8/UInt/.../Byte/...）。
            if matches!(
                self.env.store.kind(found),
                TypeKind::Value(crate::ty::ValueTypeKind::Int)
            ) && matches!(
                self.env.store.kind(expected),
                TypeKind::Value(
                    crate::ty::ValueTypeKind::Int
                        | crate::ty::ValueTypeKind::UInt
                        | crate::ty::ValueTypeKind::IntN(_)
                        | crate::ty::ValueTypeKind::UIntN(_)
                )
            ) {
                return true;
            }
            // 浮点字面量吸收：Float64 可赋值给 Float32。
            if matches!(
                self.env.store.kind(found),
                TypeKind::Value(crate::ty::ValueTypeKind::Float64)
            ) && matches!(
                self.env.store.kind(expected),
                TypeKind::Value(crate::ty::ValueTypeKind::Float32)
            ) {
                return true;
            }
        }
        let fk = self.env.store.kind(found);
        let ek = self.env.store.kind(expected);
        // Any：所有类型可装箱赋值给 Any（spec P2 §2.1）。
        if matches!(ek, TypeKind::Ref(crate::ty::RefTypeKind::Any)) {
            return true;
        }
        // TypeParam：leniently accept（泛型实例化推迟；签名中的类型参数可接受任何实参）。
        if matches!(fk, TypeKind::Param(_)) || matches!(ek, TypeKind::Param(_)) {
            return true;
        }
        // 星投影 `*` 作为期望类型：任何显式值都不能直接赋值（需 boxing / 显式转换）。
        if matches!(ek, TypeKind::StarProjection) {
            return false;
        }
        let ek = self.env.store.kind(expected);
        // 提取所有需递归检查的数据到 owned（释放 store 借用）。
        let nominal = (nominal_fqn_of(fk), nominal_fqn_of(ek));
        let func = match (fk, ek) {
            (
                TypeKind::Ref(crate::ty::RefTypeKind::Function(ff)),
                TypeKind::Ref(crate::ty::RefTypeKind::Function(ef)),
            ) => Some((ff.clone(), ef.clone())),
            _ => None,
        };
        let opt = match (fk, ek) {
            (
                TypeKind::Value(crate::ty::ValueTypeKind::Option(fi)),
                TypeKind::Value(crate::ty::ValueTypeKind::Option(ei)),
            ) => Some((*fi, *ei)),
            _ => None,
        };
        let tup = match (fk, ek) {
            (
                TypeKind::Value(crate::ty::ValueTypeKind::Tuple(fs)),
                TypeKind::Value(crate::ty::ValueTypeKind::Tuple(es)),
            ) => Some((fs.clone(), es.clone())),
            _ => None,
        };
        // nominal 子类型
        if let (Some(ffqn), Some(efqn)) = nominal {
            // 期望类型（ek）的 nominal 类型实参含星投影 `*` 时，
            // 引用类型与 `Option<Ref>` 可作为读视图（`Any?`），其它值类型需显式 boxing。
            let expected_has_star_arg = match ek {
                TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
                | TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => n
                    .args
                    .iter()
                    .any(|&a| matches!(self.env.store.kind(a), TypeKind::StarProjection)),
                _ => false,
            };
            if expected_has_star_arg {
                // found 的元素类型必须是引用或 Option<Ref>。
                let found_read_compatible = match fk {
                    TypeKind::Ref(crate::ty::RefTypeKind::Nominal(fn_)) => fn_
                        .args
                        .iter()
                        .all(|&a| self.is_star_read_compatible_elem(a)),
                    TypeKind::Value(crate::ty::ValueTypeKind::Nominal(fn_)) => fn_
                        .args
                        .iter()
                        .all(|&a| self.is_star_read_compatible_elem(a)),
                    _ => false,
                };
                if !found_read_compatible {
                    return false;
                }
                // 读视图兼容：FQN 匹配即可。
                return self.fqn_is_subtype(ffqn, efqn);
            }
            return self.fqn_is_subtype(ffqn, efqn);
        }
        // 内建标量的 nominal 子类型（Int → scoop.core.Int 的超类型链）。
        if let (Some(sfqn), Some(efqn)) = (scalar_fqn(fk, self.env.interner), nominal_fqn_of(ek)) {
            return self.fqn_is_subtype(sfqn, efqn);
        }
        // 函数：参数逆变 + 返回协变 + effect 行逆变（found.effects ⊆ expected.effects）。
        if let Some((ff, ef)) = func {
            return ff.params.len() == ef.params.len()
                && ff
                    .params
                    .iter()
                    .zip(&ef.params)
                    .all(|(fp, ep)| self.assignable_with(*ep, *fp, absorb))
                && self.assignable_with(ff.return_ty, ef.return_ty, absorb)
                && (ff.receiver.is_none() || ef.receiver.is_some())
                && ff.effects.is_subset_of(&ef.effects);
        }
        // Option 协变
        if let Some((fi, ei)) = opt {
            return self.assignable_with(fi, ei, absorb);
        }
        // 元组协变
        if let Some((fs, es)) = tup {
            return fs.len() == es.len()
                && fs
                    .iter()
                    .zip(&es)
                    .all(|(f, e)| self.assignable_with(*f, *e, absorb));
        }
        false
    }

    /// FQN 子类型 DFS：fqn_a == fqn_b 或 fqn_a 的超类型链包含 fqn_b。
    fn fqn_is_subtype(&self, fqn_a: Symbol, fqn_b: Symbol) -> bool {
        if fqn_a == fqn_b || self.fqn_same_simple_name(fqn_a, fqn_b) {
            return true;
        }
        // 显式超类型链。
        if self
            .env
            .index
            .supertypes_of(fqn_a)
            .iter()
            .any(|&sup| self.fqn_is_subtype(sup, fqn_b))
        {
            return true;
        }
        // 隐式 Any 根：所有引用类型（非值标量）是 Any 的子类型。
        if let Some(any_fqn) = self.env.interner.get("scoop.core.Any")
            && fqn_b == any_fqn
        {
            let a_text = self.env.interner.resolve(fqn_a);
            // 值标量不是 Any 的子类型（需 boxing）。
            let is_value_scalar = matches!(
                a_text,
                "scoop.core.Int"
                    | "scoop.core.UInt"
                    | "scoop.core.Bool"
                    | "scoop.core.Char"
                    | "scoop.core.Float32"
                    | "scoop.core.Float64"
            ) || a_text.contains("IntN")
                || a_text.contains("UIntN");
            // effect 类型不是值类型，可作为 Any。
            let is_effect = self
                .env
                .index
                .category(fqn_a)
                .is_some_and(|c| matches!(c, crate::resolve::symbol::NominalCategory::Effect));
            return !is_value_scalar || is_effect;
        }
        false
    }

    /// 两个 FQN 是否末段名相同（simple name 匹配）。
    /// 用于超类型 FQN 解析不精确时的回退匹配（collect 阶段超类型按 package 前缀解析，
    /// 跨 import 的超类型可能 FQN 不精确但 simple name 一致）。
    fn fqn_same_simple_name(&self, a: Symbol, b: Symbol) -> bool {
        let ta = self.env.interner.resolve(a);
        let tb = self.env.interner.resolve(b);
        let la = ta.rsplit('.').next();
        let lb = tb.rsplit('.').next();
        la.is_some_and(|l| l == lb.unwrap_or("") && !l.is_empty())
    }

    fn check_assignable(&mut self, found: TypeId, expected: TypeId, span: Span) {
        // 函数值擦除到 Any：必须是闭合 Pure!（open / Pure 或带 effect 的函数不可擦除）。
        if matches!(
            self.env.store.kind(expected),
            TypeKind::Ref(crate::ty::RefTypeKind::Any)
        ) && let TypeKind::Ref(crate::ty::RefTypeKind::Function(ft)) = self.env.store.kind(found)
            && !(ft.closed && ft.effects.is_pure())
        {
            self.diags
                .push(diagnostics::fn_value_to_any_requires_closed_pure(span));
            return;
        }
        // Continuation answer-type 精确比较：assignable 对同 FQN nominal 不比较类型实参，
        // 但 Continuation 的 answer type（第二类型实参）必须精确匹配（escape continuation binder
        // 携带 delimiter answer type，传递给不同 answer 的 Continuation 参数是类型错误）。
        if let (
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nf)),
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(ne)),
        ) = (self.env.store.kind(found), self.env.store.kind(expected))
        {
            let is_cont = |fqn: scoop2_base::Symbol| {
                self.env.interner.resolve(fqn).ends_with(".Continuation")
            };
            if nf.fqn == ne.fqn && is_cont(nf.fqn) && nf.args.len() >= 2 && ne.args.len() >= 2 {
                let found_answer = nf.args[1];
                let expected_answer = ne.args[1];
                if found_answer != expected_answer
                    && !self.assignable(found_answer, expected_answer)
                {
                    self.diags.push(diagnostics::call_arg_type_mismatch(
                        &self.describe(expected),
                        &self.describe(found),
                        span,
                    ));
                    return;
                }
            }
        }
        if !self.assignable(found, expected) {
            self.diags.push(diagnostics::type_mismatch(
                &self.describe(expected),
                &self.describe(found),
                span,
            ));
        }
    }

    fn unsupported(&mut self, what: &str, span: Span) -> TypeId {
        if !self.lenient_type_errors {
            self.diags
                .push(diagnostics::unsupported_in_this_phase(what, span));
        }
        self.env.store.nothing()
    }

    // ---- 语句 / 块 ----

    /// 从 `if` 条件提取 smart-cast 收窄信息：返回 `(then_narrow, else_narrow)`。
    /// `x is T` → then 分支 x 窄化为 T；else 不变。
    /// `x !is T` → else 分支 x 窄化为 T；then 不变。
    /// 仅对局部变量绑定（参数 / `val`）生效。
    #[allow(clippy::type_complexity)]
    fn extract_narrowing(
        &mut self,
        cond: &Expr,
    ) -> (
        Option<(scoop2_base::Symbol, TypeId)>,
        Option<(scoop2_base::Symbol, TypeId)>,
    ) {
        use crate::syntax::ast::{ExprKind, TypeCheckOp};
        match &cond.kind {
            ExprKind::TypeCheck {
                expr: inner,
                op,
                ty,
            } => {
                // 仅当 inner 是局部变量 Ident 时收窄。
                if let ExprKind::Ident(ident) = &inner.kind
                    && self.locals.contains_key(&ident.symbol)
                    && !self.local_vars.contains_key(&ident.symbol)
                {
                    let target = self.lower_type(ty);
                    // 仅当收窄类型是 inner 类型的子类型（或 inner 是 Any）时才有意义；
                    // 这里保守：只要 inner 当前类型可赋值给收窄目标或收窄目标是 inner 的子类型即收窄。
                    let cur = self.locals.get(&ident.symbol).copied();
                    if let Some(cur) = cur {
                        let cur_is_any = matches!(
                            self.env.store.kind(cur),
                            TypeKind::Ref(crate::ty::RefTypeKind::Any)
                        );
                        let target_subtype = self.assignable(target, cur);
                        if cur_is_any || target_subtype {
                            match op {
                                TypeCheckOp::Is => return (Some((ident.symbol, target)), None),
                                TypeCheckOp::NotIs => return (None, Some((ident.symbol, target))),
                            }
                        }
                    }
                }
                (None, None)
            }
            _ => (None, None),
        }
    }

    fn walk_block(&mut self, block: &Block) -> TypeId {
        let mut result = self.env.store.unit();
        for s in &block.stmts {
            match &s.kind {
                StmtKind::Expr(e) => result = self.walk_expr(e),
                _ => {
                    self.walk_stmt(s);
                    result = self.env.store.unit();
                }
            }
        }
        // 最后一条语句带 `;` → block 值为 Unit（expr 被当作语句，不贡献尾部值）。
        if block.last_trailing_semi {
            self.env.store.unit()
        } else {
            result
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Empty => {}
            StmtKind::Expr(e) => {
                self.walk_expr(e);
            }
            StmtKind::Assign { target, value } => {
                let val_ty = self.walk_expr(value);
                match &target.kind {
                    AssignTargetKind::Ident(ident) => {
                        // 局部 `val` 不可赋值（在 locals 但不在 local_vars）。
                        if self.locals.contains_key(&ident.symbol)
                            && !self.local_vars.contains_key(&ident.symbol)
                        {
                            self.diags.push(diagnostics::assignment_target_not_mutable(
                                self.env.interner.resolve(ident.symbol),
                                target.span,
                            ));
                        }
                        // 检测 lambda 捕获可变变量（赋值目标）。
                        if let Some(&decl_depth) = self.local_vars.get(&ident.symbol)
                            && decl_depth < self.lambda_depth
                        {
                            self.diags
                                .push(diagnostics::closure_var_capture_not_allowed(
                                    self.env.interner.resolve(ident.symbol),
                                    target.span,
                                ));
                        }
                        if let Some(&expected) = self.locals.get(&ident.symbol)
                            && !self.assignable(val_ty, expected)
                            && !self.lenient_type_errors
                        {
                            self.diags.push(diagnostics::assignment_type_mismatch(
                                &self.describe(expected),
                                &self.describe(val_ty),
                                value.span,
                            ));
                        }
                        // 非局部赋值（顶层 var / this）→ 值已 walk；目标类型检查推迟。
                    }
                    AssignTargetKind::Member { receiver, member } => {
                        let rt = self.walk_expr(receiver);
                        // 值类型（struct/enum）的字段不可直接赋值（值语义；应用 with-update）。
                        if self.env.store.is_value(rt)
                            && !self.env.store.is_nothing(rt)
                            && !self.env.store.is_unit(rt)
                        {
                            let (fname, fspan) = match member {
                                MemberName::Named(n) => {
                                    (self.env.interner.resolve(n.symbol).to_string(), n.span)
                                }
                                MemberName::TupleIndex { value, span } => {
                                    (value.to_string(), *span)
                                }
                            };
                            self.diags
                                .push(diagnostics::assignment_target_not_mutable(&fname, fspan));
                        }
                        // 引用类型的 `val` 属性不可赋值（不可变成员）。
                        if let MemberName::Named(name) = member
                            && let Some(fqn) = nominal_fqn_of(self.env.store.kind(rt))
                            && self.env.is_immutable_member(fqn, name.symbol)
                        {
                            self.diags.push(diagnostics::assignment_target_not_mutable(
                                self.env.interner.resolve(name.symbol),
                                name.span,
                            ));
                        }
                        if let MemberName::Named(name) = member
                            && let Some(fqn) = nominal_fqn_of(self.env.store.kind(rt))
                            && let Some(expected) = self.env.member_type(fqn, name.symbol)
                        {
                            self.check_assignable(val_ty, expected, value.span);
                        }
                        // 未知成员 / 非名义接收者 → 值已 walk；推迟。
                    }
                    AssignTargetKind::Index { receiver, indices } => {
                        self.walk_expr(receiver);
                        for idx in indices {
                            self.walk_expr(idx);
                        }
                        // operator set 解析推迟。
                    }
                }
            }
            StmtKind::LocalVal(val) => self.walk_local_val(val, stmt),
            StmtKind::Return { value } => {
                if !self.in_function_body {
                    self.diags
                        .push(diagnostics::return_not_in_function_body(stmt.span));
                    return;
                }
                if let Some(e) = value {
                    let t = self.walk_expr(e);
                    if let Some(expected) = self.return_ty
                        && !self.assignable(t, expected)
                        // 期望为函数类型但实参不是函数类型时跳过（`{ ... }` 块表达式应
                        // 作为零参 lambda 包装，当前 M7 未覆盖）。
                        && !(matches!(
                            self.env.store.kind(expected),
                            TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
                        ) && !matches!(
                            self.env.store.kind(t),
                            TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
                        ))
                    {
                        self.diags.push(diagnostics::return_type_mismatch(
                            &self.describe(expected),
                            &self.describe(t),
                            e.span,
                        ));
                    }
                } else if let Some(expected) = self.return_ty
                    && !self.env.store.is_unit(expected)
                {
                    self.diags.push(diagnostics::return_value_required(
                        &self.describe(expected),
                        stmt.span,
                    ));
                }
            }
            StmtKind::While { cond, body } => {
                let ct = self.walk_expr(cond);
                if !matches!(
                    self.env.store.kind(ct),
                    TypeKind::Value(crate::ty::ValueTypeKind::Bool)
                ) && !self.env.store.is_nothing(ct)
                {
                    self.diags.push(diagnostics::while_condition_not_bool(
                        &self.describe(ct),
                        cond.span,
                    ));
                }
                self.in_loop += 1;
                self.walk_block(body);
                self.in_loop -= 1;
            }
            StmtKind::For { binder, iter, body } => {
                let iter_ty = self.walk_expr(iter);
                // iterable 契约：iterator() → (next() → Option<T>)。
                self.check_for_iterable(iter_ty, iter.span);
                self.in_loop += 1;
                self.locals.insert(binder.symbol, self.env.store.nothing());
                self.walk_block(body);
                self.in_loop -= 1;
            }
            StmtKind::Break => {
                if self.in_loop == 0 {
                    self.diags.push(diagnostics::break_not_in_loop(stmt.span));
                }
            }
            StmtKind::Continue => {
                if self.in_loop == 0 {
                    self.diags
                        .push(diagnostics::continue_not_in_loop(stmt.span));
                }
            }
        }
    }

    fn walk_local_val(&mut self, val: &ast::ValDecl, stmt: &Stmt) {
        // 局部 val/var 必须有 initializer（无 definite assignment）。
        if val.init.is_none() {
            let span = match &val.binding {
                ast::ValBinding::Name(n) => n.span,
                ast::ValBinding::Pattern(_) => stmt.span,
            };
            self.diags.push(diagnostics::unsupported_expr(
                "局部 val/var（缺少 initializer）",
                span,
            ));
            return;
        }
        let declared = val.ty.as_ref().map(|t| self.lower_type(t));
        let init_ty = val.init.as_ref().map(|e| self.walk_expr(e));
        // 空数组字面量 `[]` 无显式类型标注时必须报错（无法推断元素类型）。
        if declared.is_none()
            && val
                .init
                .as_ref()
                .is_some_and(|e| matches!(&e.kind, ExprKind::ArrayLit(els) if els.is_empty()))
        {
            self.diags
                .push(diagnostics::array_lit_type_annotation_required(
                    val.init.as_ref().map(|e| e.span).unwrap_or(stmt.span),
                ));
        }
        // 函数值擦除到 Any：必须是闭合 Pure!（open / Pure 或带 effect 的函数不可擦除）。
        if let (Some(d), Some(i)) = (declared, init_ty)
            && matches!(
                self.env.store.kind(d),
                TypeKind::Ref(crate::ty::RefTypeKind::Any)
            )
            && let TypeKind::Ref(crate::ty::RefTypeKind::Function(ft)) = self.env.store.kind(i)
            && !(ft.closed && ft.effects.is_pure())
        {
            self.diags
                .push(diagnostics::fn_value_to_any_requires_closed_pure(
                    val.init.as_ref().map(|e| e.span).unwrap_or(stmt.span),
                ));
        }
        if let (Some(d), Some(i)) = (declared, init_ty)
            && !matches!(
                self.env.store.kind(i),
                TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
            )
        {
            // 显式类型实参的构造调用（`Container<String>()`）与声明类型（`Container<Int>`）
            // 同 FQN 但类型实参不同时，直接判定冲突（assignable 对同 FQN nominal 不比较类型实参）。
            let init_has_explicit_type_args = val.init.as_ref().is_some_and(|e| {
                if let ExprKind::Call { callee, .. } = &e.kind {
                    matches!(&callee.kind, ExprKind::TypeApply { args, .. } if !args.is_empty())
                } else {
                    false
                }
            });
            let same_fqn_diff_args = match (self.env.store.kind(i), self.env.store.kind(d)) {
                (
                    TypeKind::Ref(crate::ty::RefTypeKind::Nominal(ni)),
                    TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nd)),
                )
                | (
                    TypeKind::Value(crate::ty::ValueTypeKind::Nominal(ni)),
                    TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nd)),
                ) => ni.fqn == nd.fqn && ni.args != nd.args,
                _ => false,
            };
            if init_has_explicit_type_args && same_fqn_diff_args {
                self.diags.push(diagnostics::initializer_type_mismatch(
                    &self.describe(d),
                    &self.describe(i),
                    stmt.span,
                ));
            } else if !self.assignable(i, d)
                && !self.is_array_lit_to_mutable_array(val.init.as_ref(), d, i)
                && !self.is_empty_array_lit_to_array(val.init.as_ref(), d, i)
            {
                // 同 FQN nominal 但类型实参不同（type arg 推断不精确）→ lenient。
                // 但若 init 是带显式类型实参的构造调用（`Container<String>()`），类型实参明确、
                // 非推断，冲突是真实错误，不跳过。
                let init_has_explicit_type_args2 = val.init.as_ref().is_some_and(|e| {
                    if let ExprKind::Call { callee, .. } = &e.kind {
                        matches!(&callee.kind, ExprKind::TypeApply { args, .. } if !args.is_empty())
                    } else {
                        false
                    }
                });
                let same_owner = match (self.env.store.kind(i), self.env.store.kind(d)) {
                    (
                        TypeKind::Ref(crate::ty::RefTypeKind::Nominal(ni)),
                        TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nd)),
                    ) => ni.fqn == nd.fqn,
                    (
                        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(ni)),
                        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nd)),
                    ) => ni.fqn == nd.fqn,
                    _ => false,
                };
                if same_owner && !init_has_explicit_type_args2 {
                    return;
                }
                self.diags.push(diagnostics::initializer_type_mismatch(
                    &self.describe(d),
                    &self.describe(i),
                    val.init.as_ref().map(|e| e.span).unwrap_or(stmt.span),
                ));
            }
        }
        let ty = declared
            .or(init_ty)
            .unwrap_or_else(|| self.env.store.nothing());
        match &val.binding {
            ValBinding::Name(name) => {
                self.locals.insert(name.symbol, ty);
                if val.kind == crate::syntax::ast::ValKind::Var {
                    self.local_vars.insert(name.symbol, self.lambda_depth);
                }
            }
            ValBinding::Pattern(p) => {
                self.bind_pattern_locals(p);
                self.check_val_pattern(p, init_ty.unwrap_or(ty));
            }
        }
    }

    /// 判断 `Array<T>` 字面量初始化器是否可赋值给 `MutableArray<T>` 声明类型（上下文相关转换）。
    /// 数组字面量 `[1, 2]` 在声明为 `MutableArray`/`MutableList` 时视为 MutableArray 构造。
    fn is_array_lit_to_mutable_array(
        &self,
        init: Option<&Expr>,
        declared: TypeId,
        init_ty: TypeId,
    ) -> bool {
        // init 必须是 ArrayLit。
        let is_array_lit = init.is_some_and(|e| matches!(e.kind, ExprKind::ArrayLit(_)));
        if !is_array_lit {
            return false;
        }
        // declared 和 init_ty 都必须是 Ref(Nominal)。
        let dk = self.env.store.kind(declared);
        let ik = self.env.store.kind(init_ty);
        let (
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(dn)),
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(in_)),
        ) = (dk, ik)
        else {
            return false;
        };
        let d_name = self.env.interner.resolve(dn.fqn);
        let i_name = self.env.interner.resolve(in_.fqn);
        // declared 是 MutableArray，init 是 Array。
        if !(d_name.ends_with(".MutableArray") && i_name.ends_with(".Array")) {
            return false;
        }
        // 元素类型必须匹配（args[0]）。
        dn.args.len() == 1 && in_.args.len() == 1 && self.assignable(in_.args[0], dn.args[0])
    }

    /// 判断空数组字面量 `[]`（init_ty = Nothing）是否可赋值给 `Array<T>` 声明类型。
    fn is_empty_array_lit_to_array(
        &self,
        init: Option<&Expr>,
        declared: TypeId,
        init_ty: TypeId,
    ) -> bool {
        let is_empty_array_lit =
            init.is_some_and(|e| matches!(&e.kind, ExprKind::ArrayLit(els) if els.is_empty()));
        if !is_empty_array_lit || !self.env.store.is_nothing(init_ty) {
            return false;
        }
        // declared 必须是 Array<T> / MutableArray<T>。
        let Some(dn) = nominal_fqn_of(self.env.store.kind(declared)) else {
            return false;
        };
        let d_name = self.env.interner.resolve(dn);
        d_name.ends_with(".Array") || d_name.ends_with(".MutableArray")
    }

    /// 校验 for 循环 iterable：iterator() → (next() → Option<T>)。
    fn check_for_iterable(&mut self, iter_ty: TypeId, span: Span) {
        let Some(iter_fqn) = nominal_fqn_of(self.env.store.kind(iter_ty)) else {
            return;
        };
        let iterator_sym = self.env.interner.get("iterator");
        let next_sym = self.env.interner.get("next");
        // iterator() 必须存在。
        let Some(it_ret) = iterator_sym.and_then(|m| self.env.member_signatures(iter_fqn, m))
        else {
            self.diags
                .push(diagnostics::for_missing_iterator_method(span));
            return;
        };
        let Some(it_sig) = it_ret.first() else {
            self.diags
                .push(diagnostics::for_missing_iterator_method(span));
            return;
        };
        let it_ret_ty = it_sig.return_ty;
        // iterator() 返回类型必须有 next()。
        let Some(it_ret_fqn) = nominal_fqn_of(self.env.store.kind(it_ret_ty)) else {
            return;
        };
        let Some(next_sigs) = next_sym.and_then(|m| self.env.member_signatures(it_ret_fqn, m))
        else {
            self.diags.push(diagnostics::for_missing_next_method(span));
            return;
        };
        let Some(next_sig) = next_sigs.first() else {
            self.diags.push(diagnostics::for_missing_next_method(span));
            return;
        };
        // next() 返回必须是 Option<T>。
        if !matches!(
            self.env.store.kind(next_sig.return_ty),
            TypeKind::Value(crate::ty::ValueTypeKind::Option(_))
        ) {
            self.diags.push(diagnostics::for_next_not_option(span));
        }
    }

    /// 校验 `GC.handleNew/handleGet/handleDrop` 的参数契约。
    fn check_gc_handle_call(&mut self, method: &str, arg_ty: TypeId, span: Span) {
        let is_ref = matches!(self.env.store.kind(arg_ty), TypeKind::Ref(_));
        let is_gchandle = nominal_fqn_of(self.env.store.kind(arg_ty))
            .map(|f| self.env.interner.resolve(f).ends_with(".GcHandle"))
            .unwrap_or(false);
        let is_pinned = nominal_fqn_of(self.env.store.kind(arg_ty))
            .map(|f| self.env.interner.resolve(f).ends_with(".Pinned"))
            .unwrap_or(false);
        match method {
            "handleNew" if !is_ref => {
                self.diags
                    .push(diagnostics::gc_handle_new_requires_ref(span));
            }
            "handleGet" if !is_gchandle => {
                self.diags
                    .push(diagnostics::gc_handle_get_requires_handle(span));
            }
            "handleDrop" if !is_gchandle => {
                self.diags
                    .push(diagnostics::gc_handle_drop_requires_handle(span));
            }
            // GC.unpin 的参数必须是 Pinned token（固定令牌）。
            "unpin" if !is_pinned => {
                self.diags.push(diagnostics::gc_unpin_requires_ref(span));
            }
            // GC.pin 的参数必须是 GC 引用。
            "pin" if !is_ref => {
                self.diags.push(diagnostics::gc_pin_requires_ref(span));
            }
            _ => {}
        }
    }

    /// 泛型函数调用的 where 约束检查：从位置实参推断类型实参，检查每个约束。
    fn check_generic_call_constraints(
        &mut self,
        fqn: Symbol,
        sig: &Signature,
        arg_types: &[TypeId],
        span: Span,
    ) {
        use crate::syntax::ast::GenericBound;
        // 推断：param[i] 若为 Param(p) → p.name = arg_types[i]。
        let mut subst: HashMap<Symbol, TypeId> = HashMap::new();
        for (i, p) in sig.params.iter().enumerate() {
            if let TypeKind::Param(tp) = self.env.store.kind(*p)
                && let Some(&arg) = arg_types.get(i)
            {
                subst.insert(tp.name, arg);
            }
        }
        // 也从显式类型实参推断（`RefBox<U>` 中的 U → explicit_type_args）。
        // 这是构造器调用的路径——显式类型实参绑定到声明的类型参数。
        let Some((_tp_names, constraints)) = self.env.type_constraints(fqn) else {
            return;
        };
        // 若调用是构造器且有显式类型实参，从 tp_names 与 explicit_type_args 推断 subst。
        // 但 check_generic_call_constraints 的签名不含 explicit_type_args；
        // 对构造器调用，type args 已通过 ctor_call → check_call_args_with 处理。
        // 这里仅需处理函数调用的约束检查。
        let constraints: Vec<(Symbol, GenericBound)> = constraints.to_vec();
        for (name, bound) in &constraints {
            let Some(&arg) = subst.get(name) else {
                continue;
            };
            let violated = match bound {
                GenericBound::Ref(_) => match self.env.store.kind(arg) {
                    TypeKind::Ref(_) => false,
                    // 类型参数：只有声明了 `ref` bound 才满足 `ref` 约束（forward）。
                    TypeKind::Param(p) => !self.param_has_ref_bound(p.name),
                    _ => true,
                },
                GenericBound::Value(_) => match self.env.store.kind(arg) {
                    TypeKind::Value(_) => false,
                    // 类型参数：只有声明了 `value` bound 才满足 `value` 约束（forward）。
                    TypeKind::Param(p) => !self.param_has_value_bound(p.name),
                    _ => true,
                },
                GenericBound::Type(_) => !self.arg_satisfies_type_bound(arg, bound),
            };
            if violated {
                self.diags.push(diagnostics::where_constraint_not_satisfied(
                    &generic_bound_desc(bound, self.env.interner),
                    span,
                ));
                return;
            }
        }
    }

    /// 检查当前函数的 type_params 中，名为 `name` 的类型参数是否声明了 `ref` bound。
    fn param_has_ref_bound(&self, name: scoop2_base::Symbol) -> bool {
        self.param_ref_bounds.contains(&name)
    }

    /// 检查当前函数的 type_params 中，名为 `name` 的类型参数是否声明了 `value` bound。
    fn param_has_value_bound(&self, name: scoop2_base::Symbol) -> bool {
        self.param_value_bounds.contains(&name)
    }

    /// 实参是否满足一个 Type bound（子类型 / TypeParam lenient）。
    fn arg_satisfies_type_bound(
        &mut self,
        arg: TypeId,
        bound: &crate::syntax::ast::GenericBound,
    ) -> bool {
        use crate::syntax::ast::GenericBound;
        let GenericBound::Type(t) = bound else {
            return true;
        };
        let bound_ty = self.lower_type(t);
        if arg == bound_ty {
            return true;
        }
        if matches!(self.env.store.kind(arg), TypeKind::Param(_))
            || matches!(self.env.store.kind(bound_ty), TypeKind::Param(_))
        {
            return true;
        }
        let arg_fqn = nominal_fqn_of(self.env.store.kind(arg))
            .or_else(|| scalar_fqn(self.env.store.kind(arg), self.env.interner));
        let bound_fqn = nominal_fqn_of(self.env.store.kind(bound_ty));
        if let (Some(a), Some(b)) = (arg_fqn, bound_fqn) {
            return self.env.fqn_is_subtype(a, b);
        }
        true
    }

    /// `when` 缺少的分支（None = 穷尽；Some(missing) = 缺少的 case 描述）。
    fn when_missing_case(&self, subject_ty: TypeId, arms: &[ast::WhenArm]) -> Option<String> {
        use crate::syntax::ast::PatternKind;
        // else / 通配 / 绑定 → 穷尽。
        if arms.iter().any(|a| {
            matches!(
                a.pat.kind,
                PatternKind::Else | PatternKind::Wildcard | PatternKind::Bind(_)
            )
        }) {
            return None;
        }
        // 带守卫的分支不保证覆盖（守卫可能失败），排除后做递归穷尽性检查。
        let unguarded_pats: Vec<&ast::Pattern> = arms
            .iter()
            .filter(|a| a.guard.is_none())
            .map(|a| &a.pat)
            .collect();
        self.missing_pattern(subject_ty, &unguarded_pats)
    }

    /// 递归穷尽性检查：给定类型与一组覆盖它的（无守卫）模式，返回一个未覆盖的示例
    /// （`None` 表示已穷尽）。
    ///
    /// 覆盖的类型域：Bool（true/false）、Option<T>（None + 穷尽 T 的 Some）、
    /// Tuple（各位置组合穷尽）、enum（所有 variant，带 payload 的 variant 需穷尽 payload）。
    /// 其余类型（标量 / 无穷值域 nominal）必须有通配/绑定模式，否则需 `else`。
    fn missing_pattern(&self, ty: TypeId, pats: &[&ast::Pattern]) -> Option<String> {
        use crate::syntax::ast::{PatternKind, PatternLiteral};
        // 通配 / 绑定 / else / or-pattern 中含通配 → 覆盖全部。
        if pats.iter().any(|p| self.pattern_covers_all(p)) {
            return None;
        }
        match self.env.store.kind(ty) {
            TypeKind::Value(crate::ty::ValueTypeKind::Bool) => {
                let mut has_true = false;
                let mut has_false = false;
                for p in pats {
                    if let PatternKind::Literal(PatternLiteral::Bool { value, .. }) = &p.kind {
                        if *value {
                            has_true = true;
                        } else {
                            has_false = true;
                        }
                    }
                }
                if !has_true {
                    return Some("true".to_string());
                }
                if !has_false {
                    return Some("false".to_string());
                }
                None
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Option(inner_ty)) => {
                let inner = *inner_ty;
                let mut has_none = false;
                let mut some_inner_wild = false;
                let mut some_inner_pats: Vec<&ast::Pattern> = Vec::new();
                for p in pats {
                    if let PatternKind::Variant { path, args } = &p.kind
                        && let Some(seg) = path.segments.last()
                    {
                        let name = self.env.interner.resolve(seg.symbol);
                        if name == "None" {
                            has_none = true;
                        } else if name == "Some" {
                            match args {
                                // bare Some 或空括号 Some() → 内层通配（覆盖全部）。
                                None => some_inner_wild = true,
                                Some(a) if a.is_empty() => some_inner_wild = true,
                                Some(a) => {
                                    if let Some(first) = a.first() {
                                        if self.pattern_covers_all(first) {
                                            some_inner_wild = true;
                                        } else {
                                            some_inner_pats.push(first);
                                        }
                                    } else {
                                        some_inner_wild = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if !has_none {
                    return Some("None".to_string());
                }
                if !some_inner_wild && some_inner_pats.is_empty() {
                    return Some("Some(_)".to_string());
                }
                if some_inner_wild {
                    return None;
                }
                // Some 的内层需穷尽 inner。
                self.missing_pattern(inner, &some_inner_pats)
                    .map(|m| format!("Some({m})"))
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Tuple(elem_tys)) => {
                self.missing_tuple(ty, elem_tys, pats)
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
            | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => {
                // enum：需覆盖所有 variant；带 payload 的 variant 需穷尽其 payload 类型。
                let fqn = n.fqn;
                let variants = self.env.enum_variants(fqn)?;
                // 按 variant 名收集各 arm 的 variant pattern（含 bare 形式）。
                // 同时记录出现在 or-pattern 中的 variant（展平）。
                let mut by_variant: HashMap<Symbol, Vec<VariantPatRef<'_>>> = HashMap::new();
                for p in pats {
                    for vp in self.variant_patterns_in(p) {
                        if let Some(seg) = vp.path.segments.last()
                            && variants.contains(&seg.symbol)
                        {
                            by_variant.entry(seg.symbol).or_default().push(vp);
                        }
                    }
                }
                for v in variants {
                    let arms_v = by_variant.get(v).map(|x| x.as_slice()).unwrap_or(&[]);
                    if arms_v.is_empty() {
                        return Some(self.env.interner.resolve(*v).to_string());
                    }
                    // 该 variant 的 payload 需穷尽。payload 类型：若 variant 有 payload
                    // 字段，按字段类型构造一个 tuple 做组合检查；无 payload 则已覆盖。
                    if let Some(arity) = self.env.enum_variant_arity(fqn, *v) {
                        if arity == 0 {
                            continue;
                        }
                        // 收集该 variant 各 arm 的 args（展平为内层 pattern 列表）。
                        let arg_lists: Vec<&[ast::Pattern]> = arms_v
                            .iter()
                            .map(|vp| vp.args.as_deref().unwrap_or(&[]))
                            .collect();
                        // variant 被该 arm 覆盖的条件（满足其一）：
                        //  - args 含 `..` rest（忽略剩余字段，覆盖全部）；
                        //  - args 数 == arity 且全部为通配/binder。
                        let any_covering = arg_lists.iter().any(|a| {
                            let has_rest = a.iter().any(|e| matches!(e.kind, PatternKind::Rest));
                            if has_rest {
                                return true;
                            }
                            a.len() == arity && a.iter().all(|e| self.pattern_covers_all(e))
                        });
                        if !any_covering {
                            return Some(self.env.interner.resolve(*v).to_string());
                        }
                    }
                }
                None
            }
            _ => {
                // 其余已知类型（标量 / 无穷值域）→ 需 else/通配。
                if self.env.store.is_nothing(ty) {
                    None
                } else {
                    Some("else".to_string())
                }
            }
        }
    }

    /// 元组穷尽性：按「首列分组」递归。对每个位置，所有 arm 在该位置的模式必须组合穷尽。
    fn missing_tuple(
        &self,
        _ty: TypeId,
        elem_tys: &[TypeId],
        pats: &[&ast::Pattern],
    ) -> Option<String> {
        use crate::syntax::ast::PatternKind;
        if elem_tys.is_empty() {
            return None;
        }
        let first_ty = elem_tys[0];
        // 收集首元素模式：对每个 tuple arm 取第 0 个元素（或整个 tuple 是通配 → 覆盖全部）。
        // 同时记录对应 arm 的「剩余位置」模式（第 1.. 元素），用于递归。
        // 按首元素的「等价类」分组（Bool true/false、Option Some/None、enum variant、通配）。
        // 简化但足以覆盖常见组合：把首元素是通配/binder 的 arm 视为覆盖所有首列值，
        // 并把它的剩余位置贡献给每个等价类。
        //
        // 我们对首列类型递归：先判断首列是否被穷尽（用所有 arm 的首元素模式）。
        let first_pats: Vec<&ast::Pattern> = pats
            .iter()
            .filter_map(|p| match &p.kind {
                PatternKind::Tuple(elems) => elems.first(),
                // tuple 位置的非 tuple 模式（如通配）→ 贡献通配。
                _ => None,
            })
            .collect();
        // 首列是否含通配（来自 tuple arm 的第 0 个为通配，或非 tuple 通配 arm）。
        let first_has_wild = pats.iter().any(|p| match &p.kind {
            PatternKind::Tuple(elems) => elems.first().is_some_and(|e| self.pattern_covers_all(e)),
            other => self.pattern_covers_all_pattern(other),
        });
        // 若首列未穷尽，直接返回首列缺失示例（带括号）。
        if !first_has_wild && let Some(m) = self.missing_pattern(first_ty, &first_pats) {
            return Some(format!("({}, ...)", m));
        }
        // 首列穷尽后，检查每个「首列等价类」下的剩余位置是否穷尽。
        // 把 arm 按「首列是否通配」分两组：通配组对剩余位置贡献通配；具体组按值分组。
        // 简化：对剩余位置（第 1 个元素）递归——用所有「首列通配」arm 的第 1 元素（视为通配）
        // 加上所有「首列具体值匹配某等价类」arm 的第 1 元素。
        // 这里采用更直接的充分条件：若存在首列通配 arm，则剩余位置只需被「所有首列具体 arm
        // 的并集 + 通配」穷尽。为正确处理 (Option,Bool) 这类，我们对每个首列等价类分别递归。
        let rest_tys = &elem_tys[1..];
        if rest_tys.is_empty() {
            return None;
        }
        // 首列等价类（Bool→{true,false}、Option→{None,Some}、enum→variants）。
        let classes = self.equivalence_classes(first_ty);
        if classes.is_empty() {
            // 首列为无穷值域（标量等）。此时首列必须被通配覆盖（first_has_wild），
            // 且剩余位置必须被「所有 arm 的剩余模式」组合穷尽——因为首列通配意味着
            // 任意首列值都落到同一组 arm，剩余位置覆盖取决于这些 arm 的剩余。
            if !first_has_wild {
                return None;
            }
            let rest_pats: Vec<&ast::Pattern> = pats
                .iter()
                .filter_map(|p| match &p.kind {
                    PatternKind::Tuple(elems) => elems.get(1),
                    _ => None,
                })
                .collect();
            return self
                .missing_pattern(rest_tys[0], &rest_pats)
                .map(|m| format!("(_, {m})"));
        }
        // 对每个首列等价类，收集「首列匹配该类」的 arm 的剩余模式；同时记录是否有
        // 「首列通配」arm（其剩余位置贡献通配，即该类剩余位置全覆盖）。
        for class in &classes {
            let mut rest_pats: Vec<&ast::Pattern> = Vec::new();
            let mut rest_wild = false;
            for p in pats {
                match &p.kind {
                    PatternKind::Tuple(elems) => {
                        let first = elems.first();
                        let rest: Vec<&ast::Pattern> = elems.iter().skip(1).collect();
                        if let Some(fp) = first {
                            if self.pattern_covers_all(fp) {
                                // 首列通配 → 剩余贡献通配。
                                rest_wild = true;
                            } else if self.first_pattern_matches_class(fp, class) {
                                rest_pats.extend(rest.iter().copied());
                            }
                        }
                    }
                    other => {
                        if self.pattern_covers_all_pattern(other) {
                            rest_wild = true;
                        }
                    }
                }
            }
            if rest_wild {
                continue;
            }
            if let Some(m) = self.missing_pattern(rest_tys[0], &rest_pats) {
                return Some(format!("({}, {})", class, m));
            }
        }
        None
    }

    /// 该 pattern 是否覆盖其位置的全部值（通配 / 绑定 / else；or-pattern 含通配分支）。
    fn pattern_covers_all(&self, p: &ast::Pattern) -> bool {
        self.pattern_covers_all_pattern(&p.kind)
    }

    /// pattern（按 kind）是否覆盖全部值：通配 / 绑定 / else，或 or-pattern 任一分支覆盖全部。
    fn pattern_covers_all_pattern(&self, kind: &ast::PatternKind) -> bool {
        use crate::syntax::ast::PatternKind;
        match kind {
            PatternKind::Wildcard | PatternKind::Bind(_) | PatternKind::Else => true,
            PatternKind::Or(alts) => alts
                .iter()
                .any(|a| self.pattern_covers_all_pattern(&a.kind)),
            _ => false,
        }
    }

    /// 从一个 pattern（可能是 or-pattern）中提取所有 variant pattern 引用。
    fn variant_patterns_in<'p>(&self, p: &'p ast::Pattern) -> Vec<VariantPatRef<'p>> {
        use crate::syntax::ast::PatternKind;
        let mut out = Vec::new();
        match &p.kind {
            PatternKind::Or(alts) => {
                for a in alts {
                    self.collect_variant_refs(a, &mut out);
                }
            }
            _ => self.collect_variant_refs(p, &mut out),
        }
        out
    }

    fn collect_variant_refs<'p>(&self, p: &'p ast::Pattern, out: &mut Vec<VariantPatRef<'p>>) {
        use crate::syntax::ast::PatternKind;
        if let PatternKind::Variant { path, args } = &p.kind {
            out.push(VariantPatRef { path, args });
        }
    }

    /// 首列等价类（用于 tuple 组合穷尽）：Bool→["true","false"]、Option→["None","Some(_)"]、
    /// enum→各 variant 名。无穷值域返回空。
    fn equivalence_classes(&self, ty: TypeId) -> Vec<String> {
        use crate::syntax::ast::PatternKind;
        let _ = PatternKind::Wildcard; // 抑制未用 import 警告
        match self.env.store.kind(ty) {
            TypeKind::Value(crate::ty::ValueTypeKind::Bool) => {
                vec!["true".to_string(), "false".to_string()]
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Option(_)) => {
                vec!["None".to_string(), "Some(_)".to_string()]
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
            | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => self
                .env
                .enum_variants(n.fqn)
                .map(|vs| {
                    vs.iter()
                        .map(|v| self.env.interner.resolve(*v).to_string())
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// 首列 pattern 是否匹配某等价类（字符串描述）。
    fn first_pattern_matches_class(&self, p: &ast::Pattern, class: &str) -> bool {
        use crate::syntax::ast::{PatternKind, PatternLiteral};
        match &p.kind {
            PatternKind::Literal(PatternLiteral::Bool { value, .. }) => {
                class == if *value { "true" } else { "false" }
            }
            PatternKind::Variant { path, .. } => path
                .segments
                .last()
                .map(|s| {
                    let name = self.env.interner.resolve(s.symbol);
                    name == class
                        || (name == "Some" && class == "Some(_)")
                        || (name == "None" && class == "None")
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    // ---- 表达式 ----

    /// 表达式入口：调用 [`walk_expr_inner`] 完成推断，并把结果 `expr.id → ty`
    /// 写回 [`expr_types`](ExprChecker::expr_types) 侧表，供 typed-HIR / dump-hir 消费。
    /// 所有表达式类型记录都经此单一漏斗，保证不漏节点。
    fn walk_expr(&mut self, expr: &Expr) -> TypeId {
        let ty = self.walk_expr_inner(expr);
        self.expr_types.set(expr.id, ty);
        ty
    }

    fn walk_expr_inner(&mut self, expr: &Expr) -> TypeId {
        match &expr.kind {
            ExprKind::Ident(ident) => {
                let name = self.env.interner.resolve(ident.symbol);
                if name == "true" || name == "false" {
                    return self.env.store.bool();
                }
                if name == "this" {
                    if let Some(t) = self.this_ty {
                        return t;
                    }
                    return self.unsupported("`this` 在非成员上下文", expr.span);
                }
                // 隐式 `it` 参数（无参 lambda 内的裸标识符）。
                if name == "it"
                    && let Some(it_sym) = self.env.interner.get("it")
                    && let Some(&t) = self.locals.get(&it_sym)
                {
                    return t;
                }
                if let Some(&t) = self.locals.get(&ident.symbol) {
                    // 检测 lambda 捕获可变变量。
                    if let Some(&decl_depth) = self.local_vars.get(&ident.symbol)
                        && decl_depth < self.lambda_depth
                    {
                        self.diags
                            .push(diagnostics::closure_var_capture_not_allowed(
                                name, expr.span,
                            ));
                    }
                    return t;
                }
                // 顶层 val 引用 → 返回其已降级类型。
                if let Some(t) = self.env.top_level_val(ident.symbol) {
                    return t;
                }
                // 成员函数体内：裸标识符可能是 this 的字段/属性。
                if let Some(this_ty) = self.this_ty {
                    let this_fqn = nominal_fqn_of(self.env.store.kind(this_ty))
                        .or_else(|| scalar_fqn(self.env.store.kind(this_ty), self.env.interner));
                    if let Some(fqn) = this_fqn
                        && let Some(t) = self.env.member_type(fqn, ident.symbol)
                    {
                        return t;
                    }
                }
                // 泛型函数作为值使用（无类型实参）→ 无法推断类型实参。
                if let Some(ResolvedValue::TopLevelFun { fqn }) =
                    self.resolution.value_refs.get(expr.id).copied()
                    && let Some(sigs) = self.env.signatures(fqn)
                    && sigs.iter().any(|s| s.type_param_count > 0)
                {
                    self.diags
                        .push(diagnostics::generic_type_arg_not_inferred(expr.span));
                }
                // 顶层值 / 函数引用 / 类型名 → 推迟精确类型（返回 Nothing，避免假错误）。
                self.env.store.nothing()
            }
            ExprKind::IntLit(_) => self.env.store.int(),
            ExprKind::FloatLit(lit) => {
                if lit.suffix.is_some() {
                    self.env.store.float32()
                } else {
                    self.env.store.float64()
                }
            }
            ExprKind::CharLit(_) => self.env.store.char(),
            ExprKind::StringLit(_) => self.env.store.string(),
            ExprKind::UnitLit => self.env.store.unit(),
            ExprKind::InterpolatedString { parts, .. } => {
                for p in parts {
                    if let ast::StringPart::Expr(e) = p {
                        let t = self.walk_expr(e);
                        if !self.type_implements_tostring(t) {
                            self.diags
                                .push(diagnostics::interpolation_expr_not_to_string(e.span));
                        }
                    }
                }
                self.env.store.string()
            }
            ExprKind::Binary { lhs, op, rhs } => {
                let lt = self.walk_expr(lhs);
                let rt = self.walk_expr(rhs);
                let cat = scalar_cat(self.env.store.kind(lt));
                let bool_ty = self.env.store.bool();
                match builtin_binop(*op, cat, lt, rt, bool_ty) {
                    Some(t) => t,
                    None => {
                        // 尝试运算符重载方法调用（a.plus(b) 等）。
                        if let Some(m) =
                            binop_to_method_name(*op).and_then(|n| self.env.interner.get(n))
                        {
                            let has_method = nominal_fqn_of(self.env.store.kind(lt))
                                .or_else(|| scalar_fqn(self.env.store.kind(lt), self.env.interner))
                                .is_some_and(|f| self.env.member_signatures(f, m).is_some());
                            if has_method {
                                // 运算符位置调用（`a + b`）要求方法声明了 `operator` modifier。
                                if let Some(f) =
                                    nominal_fqn_of(self.env.store.kind(lt)).or_else(|| {
                                        scalar_fqn(self.env.store.kind(lt), self.env.interner)
                                    })
                                    && let Some(sigs) = self.env.member_signatures(f, m)
                                    && sigs.iter().all(|s| {
                                        !s.modifiers
                                            .contains(crate::syntax::ast::ModifierKind::Operator)
                                    })
                                {
                                    let sym = binop_symbol(*op);
                                    let start = lhs.span.end + 1;
                                    self.diags.push(diagnostics::operator_modifier_required(
                                        sym,
                                        Span::new(start, start + sym.len()),
                                    ));
                                }
                                return self.method_call_return_type(
                                    lt,
                                    m,
                                    &[rt],
                                    &[],
                                    expr.span,
                                    expr.span,
                                    false,
                                );
                            }
                            // 接收者是已知 nominal 但缺运算符方法 → 报错。
                            if !self.env.store.is_nothing(lt)
                                && !matches!(self.env.store.kind(lt), TypeKind::Param(_))
                                && nominal_fqn_of(self.env.store.kind(lt)).is_some()
                            {
                                let sym = binop_symbol(*op);
                                let start = lhs.span.end + 1;
                                self.diags.push(diagnostics::operator_overload_not_found(
                                    sym,
                                    Span::new(start, start + sym.len()),
                                ));
                            } else if !self.env.store.is_nothing(lt)
                                && !matches!(self.env.store.kind(lt), TypeKind::Param(_))
                                && scalar_fqn(self.env.store.kind(lt), self.env.interner).is_some()
                            {
                                // 标量原始类型（如 Bool）对该运算符无内建支持 → 操作数类型不匹配。
                                let sym = binop_symbol(*op);
                                let start = lhs.span.end + 1;
                                self.diags
                                    .push(diagnostics::binary_op_operand_type_mismatch(
                                        sym,
                                        &self.describe(lt),
                                        Span::new(start, start + sym.len()),
                                    ));
                            }
                        }
                        self.env.store.nothing()
                    }
                }
            }
            ExprKind::Unary { op, expr: e } => {
                let t = self.walk_expr(e);
                let cat = scalar_cat(self.env.store.kind(t));
                let bool_ty = self.env.store.bool();
                match builtin_unary(*op, cat, t, bool_ty) {
                    Some(t) => t,
                    None => {
                        // 尝试运算符重载方法调用（a.unaryMinus() / a.inv() 等）。
                        let method = unop_to_method_name(*op);
                        if let Some(m) = method.and_then(|n| self.env.interner.get(n)) {
                            let has_method = nominal_fqn_of(self.env.store.kind(t))
                                .or_else(|| scalar_fqn(self.env.store.kind(t), self.env.interner))
                                .is_some_and(|f| self.env.member_signatures(f, m).is_some());
                            if has_method {
                                return self.method_call_return_type(
                                    t,
                                    m,
                                    &[],
                                    &[],
                                    expr.span,
                                    expr.span,
                                    false,
                                );
                            }
                            if !self.env.store.is_nothing(t)
                                && !matches!(self.env.store.kind(t), TypeKind::Param(_))
                                && nominal_fqn_of(self.env.store.kind(t)).is_some()
                            {
                                let sym = unop_symbol(*op);
                                let start = e.span.start.saturating_sub(1);
                                self.diags
                                    .push(diagnostics::unary_operator_overload_not_found(
                                        sym,
                                        Span::new(start, start + sym.len()),
                                    ));
                            }
                        }
                        self.env.store.nothing()
                    }
                }
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let ct = self.walk_expr(cond);
                if !matches!(
                    self.env.store.kind(ct),
                    TypeKind::Value(crate::ty::ValueTypeKind::Bool)
                ) && !self.env.store.is_nothing(ct)
                {
                    self.diags.push(diagnostics::if_condition_not_bool(
                        &self.describe(ct),
                        cond.span,
                    ));
                }
                // smart-cast：从条件提取 `x is T` / `x !is T` 的收窄信息。
                let (then_narrow, else_narrow) = self.extract_narrowing(cond);
                // then 分支：应用 then_narrow（x → T）。
                let saved_then = then_narrow.and_then(|(sym, ty)| {
                    self.locals.get(&sym).copied().map(|prev| {
                        self.locals.insert(sym, ty);
                        (sym, prev)
                    })
                });
                let tt = self.walk_expr(then_branch);
                if let Some((sym, prev)) = saved_then {
                    self.locals.insert(sym, prev);
                }
                match else_branch {
                    Some(e) => {
                        // else 分支：应用 else_narrow（`x !is T` 时 x → T）。
                        let saved_else = else_narrow.and_then(|(sym, ty)| {
                            self.locals.get(&sym).copied().map(|prev| {
                                self.locals.insert(sym, ty);
                                (sym, prev)
                            })
                        });
                        let et = self.walk_expr(e);
                        if let Some((sym, prev)) = saved_else {
                            self.locals.insert(sym, prev);
                        }
                        if self.assignable(et, tt) {
                            tt
                        } else if self.assignable(tt, et) {
                            et
                        } else {
                            self.env.store.unit()
                        }
                    }
                    // if-without-else 在值位为 Unit（无 else 分支产生值）。
                    None => self.env.store.unit(),
                }
            }
            ExprKind::Block(b)
            | ExprKind::DoBlock(b)
            | ExprKind::UnsafeBlock(b)
            | ExprKind::SafeBlock(b) => self.walk_block(b),
            ExprKind::Call { callee, args } => self.type_call(callee, args, expr.span),
            ExprKind::MemberAccess { receiver, member } => {
                let rt = self.walk_expr(receiver);
                // 旧 tuple 字段语法 `t._0` 已移除（应为 `t.0`）。
                if let crate::syntax::ast::MemberName::Named(n) = member {
                    let name = self.env.interner.resolve(n.symbol);
                    if name.len() >= 2
                        && name.starts_with('_')
                        && name[1..].chars().all(|c| c.is_ascii_digit())
                    {
                        self.diags.push(diagnostics::tuple_member_old_syntax(
                            name,
                            &name[1..],
                            expr.span,
                        ));
                    }
                }
                self.member_access_type(rt, *member, expr.span)
            }
            ExprKind::StructLit { name, fields } => self.type_struct_lit(*name, fields, expr.span),
            ExprKind::TupleLit(els) => {
                let types: Vec<TypeId> = els.iter().map(|e| self.walk_expr(e)).collect();
                self.env.store.tuple(types)
            }
            ExprKind::TypeCheck { expr: e, .. } => {
                self.walk_expr(e);
                self.env.store.bool()
            }
            ExprKind::Cast {
                expr: e, op, ty, ..
            } => {
                let operand_ty = self.walk_expr(e);
                let target = self.lower_type(ty);
                // 带非 Pure effect row 的函数类型不能参与显式 `as`/`as?`（检查目标 TypeRef
                // 与操作数已降级类型）。
                if is_effectful_function_type_ref(ty, self.env.interner)
                    || is_effectful_function_type(self.env, operand_ty)
                {
                    // 指向 `as` / `as?` 运算符（位于操作数与目标类型之间）。
                    let op_start = e.span.end + 1;
                    let op_len = match op {
                        CastOp::As => 2,
                        CastOp::AsSafe => 3,
                    };
                    self.diags
                        .push(diagnostics::effectful_function_type_cast_not_supported(
                            Span::new(op_start, op_start + op_len),
                        ));
                }
                match op {
                    CastOp::As => {
                        // value ↔ ref 转换在当前阶段拒绝。
                        if !self.env.store.is_nothing(operand_ty)
                            && self.env.store.is_value(operand_ty)
                                != self.env.store.is_value(target)
                        {
                            let op_start = e.span.end + 1;
                            self.diags
                                .push(diagnostics::invalid_cast(Span::new(op_start, op_start + 2)));
                        }
                        target
                    }
                    CastOp::AsSafe => {
                        let t = target;
                        self.env.store.option(t)
                    }
                }
            }
            ExprKind::NotNullAssert { expr: e } => {
                let t = self.walk_expr(e);
                if let TypeKind::Value(crate::ty::ValueTypeKind::Option(inner)) =
                    self.env.store.kind(t)
                {
                    *inner
                } else if self.env.store.is_nothing(t) {
                    // Nothing = 不可解析，放行。
                    t
                } else {
                    self.diags
                        .push(diagnostics::not_null_assert_operand_not_nullable(expr.span));
                    t
                }
            }
            ExprKind::Annotated {
                annotations,
                expr: e,
            } => {
                // 表达式前缀注解：`@Experimental` 等在此为非法目标。
                super::check_experimental_annotations(
                    annotations,
                    true,
                    self.env.interner,
                    self.diags,
                );
                self.walk_expr(e)
            }
            ExprKind::SafeMemberAccess { receiver, member } => {
                let rt = self.walk_expr(receiver);
                let base_ty = match self.env.store.kind(rt) {
                    TypeKind::Value(crate::ty::ValueTypeKind::Option(inner)) => *inner,
                    _ => rt,
                };
                let member_ty = self.member_access_type(base_ty, *member, expr.span);
                self.env.store.option(member_ty)
            }
            ExprKind::WithUpdate { base, updates } => self.type_with_update(base, updates),
            ExprKind::Lambda(lambda) => self.type_lambda(lambda),
            ExprKind::When { subject, arms } => self.type_when(subject, arms, expr.span),
            ExprKind::Handle {
                body,
                arms,
                finally,
            } => {
                // handle 体 + arm 体内的 effect 被 handle 捕获，不计入外层。
                // 但需记录 handle 体执行的 effect（用于 escape continuation 的 effect row），
                // 故不挂起记录，遍历后截断。
                let body_eff_start = self.performed_effects.len();
                let body_ty = self.walk_block(body);
                // handle 体执行的 effect（用于 escape continuation 的 resumed-step effect row）。
                // 排除被 handle 捕获的 effect（arm 处理的 effect op 所属的 effect 类型）——
                // 这些 effect 被 handle 消化，不应出现在 resumed-step effect row 中。
                let handled_effect_names: std::collections::HashSet<String> = arms
                    .iter()
                    .filter_map(|arm| {
                        arm.op
                            .effect_path
                            .segments
                            .last()
                            .map(|s| self.env.interner.resolve(s.symbol).to_string())
                    })
                    .collect();
                let body_effects: Vec<String> = self.performed_effects[body_eff_start..]
                    .iter()
                    .map(|(e, _)| e.clone())
                    .filter(|e| !handled_effect_names.contains(e))
                    .collect();
                // 清除 handle 体记录的 effect（不计入外层）。
                self.performed_effects.truncate(body_eff_start);
                // 为每个 arm 的 escape continuation binder 绑定正确的 Continuation 类型。
                // Continuation<Resume, Answer, eff Row>：
                //   Resume = effect operation 的返回类型；Answer = body_ty；
                //   Row = handle 体执行的 effect（resumed-step effect row）。
                for arm in arms {
                    if let Some(k_ident) = &arm.escape_continuation {
                        let resume_ty = self
                            .effect_op_resume_type(&arm.op)
                            .unwrap_or_else(|| self.env.store.nothing());
                        let cont_ty = self.build_continuation_type_with_effects(
                            resume_ty,
                            body_ty,
                            &body_effects,
                        );
                        self.locals.insert(k_ident.symbol, cont_ty);
                    }
                }
                // arm 体 + finally 的 effect 被 handle 捕获（不计入外层）。
                self.effect_suspend_depth += 1;
                let arm_tys: Vec<TypeId> = arms.iter().map(|a| self.walk_expr(&a.body)).collect();
                if let Some(f) = finally {
                    self.walk_block(f);
                }
                self.effect_suspend_depth -= 1;
                // handler arm 一致性检查：
                // 1. 重复 arm（同一 effect operation 已被前面 arm 覆盖）→ 不可达。
                let mut seen_ops: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for arm in arms {
                    let key = handle_arm_op_key(&arm.op, self.env.interner);
                    if let Some(k) = key
                        && !seen_ops.insert(k)
                    {
                        self.diags
                            .push(diagnostics::handle_arm_unreachable(arm.arrow_span));
                    }
                }
                // 1b. catch arm（Raise.raise 的 non-resuming arm，binder 带类型）子类型覆盖：
                //     若某 catch 的 binder 类型是前面任一 catch binder 类型的子类型，则该 catch 不可达
                //     （更宽的类型写在前面会吞掉后续更窄分支）。
                let mut prior_catch_types: Vec<TypeId> = Vec::new();
                for arm in arms {
                    if arm.escape_continuation.is_none()
                        && is_raise_raise_op(&arm.op, self.env.interner)
                        && let Some(binder) = arm.op.binders.first()
                        && let Some(ty_ref) = &binder.ty
                    {
                        let bt = self.lower_type(ty_ref);
                        // 若 bt 是前面任一 catch 类型的子类型 → 不可达。
                        let dominated = prior_catch_types
                            .iter()
                            .any(|&prev| self.assignable(bt, prev));
                        if dominated {
                            self.diags
                                .push(diagnostics::handle_arm_unreachable(arm.span));
                        }
                        prior_catch_types.push(bt);
                    }
                }
                // 2. arm 返回类型与 handle 结果类型（body_ty）一致（non-resuming / escape arm）。
                //    resuming arm（`, k ->` 且体调用 k.resume）的返回类型即 handle 结果类型，
                //    不单独检查；non-resuming arm 的返回类型必须可赋值给 body_ty。
                //    body_ty 为 Nothing（未解析）时跳过，避免假阳性。
                for (arm, &arm_ty) in arms.iter().zip(arm_tys.iter()) {
                    if arm.escape_continuation.is_none()
                        && !self.env.store.is_nothing(arm_ty)
                        && !self.env.store.is_nothing(body_ty)
                        && !self.assignable(arm_ty, body_ty)
                    {
                        self.diags
                            .push(diagnostics::handle_arm_return_type_mismatch(
                                &self.describe(body_ty),
                                &self.describe(arm_ty),
                                arm.body.span,
                            ));
                    }
                }
                body_ty
            }
            ExprKind::TypeApply { callee, .. } => self.walk_expr(callee),
            ExprKind::Index { receiver, indices } => {
                let rt = self.walk_expr(receiver);
                let idx_types: Vec<TypeId> = indices.iter().map(|i| self.walk_expr(i)).collect();
                let Some(get_sym) = self.env.interner.get("get") else {
                    return self.unsupported("下标访问（operator get 未注册）", expr.span);
                };
                self.method_call_return_type(
                    rt,
                    get_sym,
                    &idx_types,
                    &[],
                    expr.span,
                    expr.span,
                    false,
                )
            }
            ExprKind::InfixCall {
                receiver,
                name,
                arg,
            } => {
                let rt = self.walk_expr(receiver);
                let at = self.walk_expr(arg);
                self.method_call_return_type(
                    rt,
                    name.symbol,
                    &[at],
                    &[],
                    expr.span,
                    name.span,
                    false,
                )
            }
            ExprKind::ClassLit { .. } => self.env.store.string(),
            ExprKind::ArrayLit(els) => {
                if els.is_empty() {
                    // 空数组字面量 `[]`：返回 Nothing（由调用方 check_assignable 判定）。
                    return self.env.store.nothing();
                }
                let elem_ty = self.walk_expr(&els[0]);
                for (idx, e) in els[1..].iter().enumerate() {
                    let et = self.walk_expr(e);
                    // Float 字面量有吸收语义（Float64 默认可吸收为 Float32），跨 Float 类型
                    // 不做元素一致性检查；仅捕获明显的跨类别不匹配（如 Int vs String）。
                    let both_float = scalar_cat(self.env.store.kind(elem_ty)) == ScalarCat::Float
                        && scalar_cat(self.env.store.kind(et)) == ScalarCat::Float;
                    if !both_float
                        && !self.assignable(et, elem_ty)
                        && !self.env.store.is_nothing(et)
                        && !self.env.store.is_nothing(elem_ty)
                    {
                        self.diags
                            .push(diagnostics::array_lit_element_type_mismatch(
                                idx + 2,
                                &self.describe(elem_ty),
                                &self.describe(et),
                                e.span,
                            ));
                    }
                }
                let array_fqn = self
                    .env
                    .interner
                    .get("scoop.core.Array")
                    .or_else(|| self.env.interner.get("Array"));
                match array_fqn {
                    Some(fqn) => {
                        let nominal = NominalType {
                            fqn,
                            args: vec![elem_ty],
                            eff: None,
                        };
                        self.env.store.ref_nominal(nominal)
                    }
                    None => self.unsupported("Array 字面量（prelude 未加载）", expr.span),
                }
            }
            // 反射形式 `value.["field"]` / `value.[name]`。
            ExprKind::SpliceField { receiver, field } => {
                let rt = self.walk_expr(receiver);
                // field 必须是字符串字面量（静态名）。
                if let ExprKind::StringLit(s) = &field.kind {
                    if let Some(fqn) = nominal_fqn_of(self.env.store.kind(rt)) {
                        let field_sym = self.env.interner.get(&s.value);
                        if let Some(sym) = field_sym
                            && self.env.member_type(fqn, sym).is_some()
                        {
                            // 已知字段：返回其类型。
                            return self
                                .env
                                .member_type(fqn, sym)
                                .unwrap_or_else(|| self.env.store.nothing());
                        }
                        // 未知字段。
                        let type_name = self.env.interner.resolve(fqn).to_string();
                        let member_str = format!("{type_name}.{}", s.value);
                        self.diags.push(diagnostics::unsupported_member_access(
                            &member_str,
                            field.span,
                        ));
                    }
                    // 未知接收者类型 → lenient。
                    return self.env.store.nothing();
                }
                // 非字符串字面量（动态名）→ splice_field_name_not_static（但 StructLit 不报——它不是 splice field）。
                let is_struct_lit = matches!(field.kind, ExprKind::StructLit { .. });
                self.walk_expr(field);
                if !is_struct_lit {
                    self.diags
                        .push(diagnostics::splice_field_name_not_static(field.span));
                }
                self.env.store.nothing()
            }
        }
    }

    // ---- 调用（M2 单候选 + M3 多候选重载解析） ----

    fn type_call(&mut self, callee: &Expr, args: &[CallArg], span: Span) -> TypeId {
        // Unwrap explicit type args `f<T>(x)` / `obj.m<T>(x)`，保留类型实参供构造器调用。
        let (callee, explicit_type_args): (&Expr, &[crate::syntax::ast::TypeArg]) =
            match &callee.kind {
                ExprKind::TypeApply {
                    callee: inner,
                    args,
                } => (inner, args),
                _ => (callee, &[]),
            };
        // spread 实参 `*expr` 类型校验：必须是 Array / tuple（MutableArray 等需 toArray() 桥接）。
        for a in args {
            if a.is_spread {
                let ty = self.walk_expr(&a.value);
                self.check_spread_arg(ty, a.span);
            }
        }
        // 1. 函数类型局部值 / 参数调用。
        if let ExprKind::Ident(ident) = &callee.kind
            && let Some(&ft) = self.locals.get(&ident.symbol)
            && let Some(f) = self.function_type_of(ft)
        {
            // receiver function：`String.(Int) -> Int` 的 receiver 作为第一个参数。
            let mut all_params = Vec::new();
            if let Some(recv) = f.receiver {
                all_params.push(recv);
            }
            all_params.extend(f.params.iter().copied());
            // receiver function 的 arity / receiver-type 检查（不受 lenient 影响）。
            if let Some(recv_ty_id) = f.receiver
                && !args.iter().any(|a| a.name.is_some())
            {
                if all_params.len() != args.len() {
                    let callee_name = self.env.interner.resolve(ident.symbol);
                    self.diags.push(diagnostics::call_arity_mismatch_detail(
                        callee_name,
                        all_params.len(),
                        args.len(),
                        span,
                    ));
                    for a in args {
                        self.walk_expr(&a.value);
                    }
                    return f.return_ty;
                }
                // receiver 类型检查。
                let recv_ty = self.walk_expr(&args[0].value);
                if !self.assignable(recv_ty, recv_ty_id) {
                    let expected_desc = self.describe(recv_ty_id);
                    let found_desc = self.describe(recv_ty);
                    self.diags.push(diagnostics::call_receiver_type_mismatch(
                        &expected_desc,
                        &found_desc,
                        args[0].value.span,
                    ));
                }
                // 其余参数类型检查。
                for (i, a) in args.iter().enumerate().skip(1) {
                    let at = self.walk_expr(&a.value);
                    if let Some(&pt) = f.params.get(i - 1)
                        && !self.assignable(at, pt)
                        && !self.lenient_type_errors
                    {
                        self.diags.push(diagnostics::call_arg_type_mismatch(
                            &self.describe(pt),
                            &self.describe(at),
                            a.value.span,
                        ));
                    }
                }
                return f.return_ty;
            }
            return self.check_call_args(f.params, f.return_ty, args, span);
        }
        // 2. 顶层函数调用（resolve 解析到 TopLevelFun）。
        if let ExprKind::Ident(ident) = &callee.kind {
            let resolved = self.resolution.value_refs.get(callee.id).copied();
            if let Some(ResolvedValue::TopLevelFun { fqn }) = resolved {
                // 若 callee 同名也是类型（构造器），且函数不适用但构造器适用，
                // 优先用构造器（`MixedTarget(1)` 选 ctor(Int) 而非 ext fun(String)）。
                if let Some(ctor_fqn) = self.callee_type_fqn(ident.symbol) {
                    // 快速检查：构造器是否适用（不产生诊断）。
                    let arg_types: Vec<TypeId> =
                        args.iter().map(|a| self.walk_expr(&a.value)).collect();
                    let ctor_applicable = self.env.ctor_signatures(ctor_fqn).is_some_and(|sigs| {
                        sigs.iter()
                            .any(|s| self.ctor_sig_applicable(s, args, &arg_types))
                    });
                    if ctor_applicable {
                        return self.ctor_call(ctor_fqn, args, span, explicit_type_args);
                    }
                }
                return self.resolve_top_level_call(fqn, args, span);
            }
            // enum variant ctor（resolve 解析到 TopLevelValue，但实际是 variant 调用）。
            if let Some(ResolvedValue::TopLevelValue { fqn }) = resolved {
                // 已知 prelude variant arity 检查。
                if let Some(expected_arity) = self.known_enum_variant_arity(fqn)
                    && expected_arity != args.len()
                    && !args.iter().any(|a| a.name.is_some())
                {
                    self.diags
                        .push(diagnostics::enum_variant_ctor_arity_mismatch(
                            expected_arity,
                            args.len(),
                            span,
                        ));
                }
                let name = self.env.interner.resolve(fqn);
                // Option 特判：Some(x) → Option(x_ty)，None → Option(Nothing)。
                // 仅对 prelude Option（`scoop.core.Option.Some/None` 或裸 `Some`/`None`）生效，
                // 不影响用户自定义 enum 或其他 enum（如 LazyThreadSafetyMode.None）中同名 variant。
                let is_prelude_some = name == "Some" || name.ends_with(".Option.Some");
                let is_prelude_none = name == "None" || name.ends_with(".Option.None");
                if is_prelude_some {
                    let inner = if args.is_empty() {
                        self.env.store.nothing()
                    } else {
                        self.walk_expr(&args[0].value)
                    };
                    let opt = self.env.store.option(inner);
                    return opt;
                }
                if is_prelude_none {
                    let nothing = self.env.store.nothing();
                    return self.env.store.option(nothing);
                }
                // 其他 enum variant：返回所属 enum 的 nominal。
                if let Some(dot) = name.rfind('.')
                    && let Some(enum_fqn) = self.env.interner.get(&name[..dot])
                {
                    // 泛型 enum variant 在无期望类型上下文时构造歧义（无法推断类型实参）。
                    // 多个 enum 拥有同名 variant 时留给期望类型消歧，不在此报歧义。
                    let variant_name = &name[dot + 1..];
                    let same_name_variant_count = self.env.count_variants_named(variant_name);
                    let enum_has_type_params = self
                        .env
                        .type_constraints(enum_fqn)
                        .is_some_and(|(tps, _)| !tps.is_empty());
                    if enum_has_type_params
                        && explicit_type_args.is_empty()
                        && same_name_variant_count <= 1
                    {
                        self.diags
                            .push(diagnostics::ambiguous_enum_variant_ctor(span));
                    }
                    let nominal = NominalType {
                        fqn: enum_fqn,
                        args: vec![],
                        eff: None,
                    };
                    return if self.env.is_reference_nominal(enum_fqn) {
                        self.env.store.ref_nominal(nominal)
                    } else {
                        self.env.store.value_nominal(nominal)
                    };
                }
                return self.env.store.nothing();
            }
        }
        // 3. 方法调用 `receiver.method(args)`。
        if let ExprKind::MemberAccess { receiver, member } = &callee.kind {
            // effect operation 检测：`Raise.raise(x)` 等 → 记录 effect。
            if let ExprKind::Ident(recv_ident) = &receiver.kind
                && let MemberName::Named(_member_name) = member
            {
                let recv_name = self.env.interner.resolve(recv_ident.symbol);
                // 检查 receiver 是否是已知 effect 类型。
                let is_effect = self.is_effect_type_name(recv_name);
                if is_effect && self.effect_suspend_depth == 0 {
                    self.performed_effects.push((recv_name.to_string(), span));
                }
            }
            let rt = if let ExprKind::Ident(recv_ident) = &receiver.kind
                && !args.iter().any(|a| a.name.is_some())
            {
                let recv_name = self.env.interner.resolve(recv_ident.symbol);
                // effect 类型名作为 receiver（`Make.make()` 的 Make）→ 返回 nominal 类型，
                // 使方法查找能定位 effect operation。
                // 仅在无命名实参时（`Echo.bump(receiver=...)` 的 receiver 命名实参
                // 是 receiver-effect-op 专用语法，不走常规方法查找）。
                if self.is_effect_type_name(recv_name)
                    && let Some(ty) = self.effect_type_to_typeid(recv_name)
                {
                    ty
                } else {
                    self.walk_expr(receiver)
                }
            } else {
                self.walk_expr(receiver)
            };
            let arg_types: Vec<TypeId> = args.iter().map(|a| self.walk_expr(&a.value)).collect();
            // Continuation.resume 步骤 effect 传播：`k.resume(v)` 的 effect 行为
            // `E + Raise<RuntimeError>`（E 为 `Continuation<..., eff E>` 的第三类型实参）。
            // resume 在 handle arm 体内执行时也需传播 effect 到外层函数体——
            // 即使 effect_suspend_depth > 0（arm body 挂起了其他 effect 采集），
            // resume 的 effect 仍需穿透到外层（resume 的 effect 是 resumed-step 的，
            // 不是 handle 捕获的）。
            if let MemberName::Named(name) = member
                && self.env.interner.resolve(name.symbol) == "resume"
            {
                self.record_continuation_resume_effects(rt, span);
                // resume 的 Resume payload 必须作为单一实参传递（tuple 不能扁平化）。
                if !self.lenient_type_errors && args.len() > 1 {
                    self.diags
                        .push(diagnostics::call_arity_mismatch(1, args.len(), span));
                }
            }
            // GC.handleNew/handleGet/handleDrop 参数契约（按 receiver 名识别 GC object）。
            if let MemberName::Named(name) = member {
                let is_gc = matches!(&receiver.kind, ExprKind::Ident(id)
                    if self.env.interner.resolve(id.symbol) == "GC");
                if is_gc
                    && let (Some(&first_arg), Some(first_call_arg)) =
                        (arg_types.first(), args.first())
                {
                    self.check_gc_handle_call(
                        self.env.interner.resolve(name.symbol),
                        first_arg,
                        first_call_arg.value.span,
                    );
                }
            }
            match member {
                MemberName::Named(name) => {
                    // FunPtr.invoke 不支持命名实参（C ABI 函数指针只能按位置调用）。
                    let is_funptr = nominal_fqn_of(self.env.store.kind(rt))
                        .map(|f| self.env.interner.resolve(f).ends_with(".FunPtr"))
                        .unwrap_or(false);
                    if is_funptr
                        && self.env.interner.resolve(name.symbol) == "invoke"
                        && args.iter().any(|a| a.name.is_some())
                    {
                        self.diags
                            .push(diagnostics::funptr_invoke_named_args_not_supported(span));
                    }
                    return self.method_call_return_type(
                        rt,
                        name.symbol,
                        &arg_types,
                        args,
                        span,
                        name.span,
                        !explicit_type_args.is_empty(),
                    );
                }
                MemberName::TupleIndex { .. } => {}
            }
        }
        // 4. 构造器调用（callee 是类型名 `Type(args)`）。
        if let ExprKind::Ident(ident) = &callee.kind
            && let Some(fqn) = self.callee_type_fqn(ident.symbol)
        {
            return self.ctor_call(fqn, args, span, explicit_type_args);
        }
        // 其他调用形式（嵌套调用、lambda 调用等）：walk args。
        for a in args {
            self.walk_expr(&a.value);
        }
        // callee 是 Ident 且不可调用（未注册函数 / 前缀标识符）→ callee_not_callable。
        if let ExprKind::Ident(ident) = &callee.kind
            && !self.lenient_type_errors
        {
            let name = self.env.interner.resolve(ident.symbol);
            if name.starts_with("__scoop_") || !name.is_empty() {
                // 检查是否是已知顶层函数 / 类型 / 值。
                let is_fun = self.resolution.value_refs.get(callee.id).is_some();
                let is_type = self.callee_type_fqn(ident.symbol).is_some();
                if !is_fun && !is_type {
                    self.diags
                        .push(diagnostics::callee_not_callable(name, span));
                }
            }
        }
        self.env.store.nothing()
    }

    /// 构造器调用 `Type(args)`：按主构造参数类型检查实参，返回该 nominal 类型。
    fn ctor_call(
        &mut self,
        fqn: Symbol,
        args: &[CallArg],
        span: Span,
        explicit_type_args: &[crate::syntax::ast::TypeArg],
    ) -> TypeId {
        // `Continuation` 是 compiler-owned interface，用户代码不能直接构造。
        let name = self.env.interner.resolve(fqn);
        let stripped = name.strip_prefix("scoop.core.").unwrap_or(name);
        if stripped == "Continuation" {
            self.diags
                .push(diagnostics::continuation_not_constructible(span));
        }
        // `object` 是单例，不能构造。
        if self
            .env
            .index
            .category(fqn)
            .is_some_and(|c| matches!(c, crate::resolve::symbol::NominalCategory::Object))
        {
            self.diags.push(diagnostics::object_not_constructible(span));
        }
        // 显式类型实参（`Container<String>()`）降级为 TypeId 列表，用于构造返回 nominal。
        let explicit_arg_types: Vec<TypeId> = explicit_type_args
            .iter()
            .filter_map(|a| match &a.kind {
                crate::syntax::ast::TypeArgKind::Type(t) => Some(self.lower_type(t)),
                _ => None,
            })
            .collect();
        // 次构造器重载决议（若存在次构造器）：适用性 + 特异性（concrete 优于 generic）。
        if let Some(ctors) = self.env.ctor_signatures(fqn).map(|c| c.to_vec())
            && !ctors.is_empty()
        {
            return self.resolve_ctor_overloads(fqn, &ctors, args, span, explicit_type_args);
        }
        // enum variant ctor：查找 enum FQN 对应的 variant 字段数。
        let params: Vec<TypeId> = self.env.ctor_params(fqn).unwrap_or(&[]).to_vec();
        // 如果 ctor_params 为空但参数不为空，可能是 enum variant ctor（未注册在 ctors 中）。
        // 尝试从 enum_variants 注册表查找。
        if params.is_empty() && !args.is_empty() {
            // 尝试找 enum variant 的 owner FQN。
            if let Some(enum_fqn) = self.find_enum_owner(fqn)
                && let Some(variants) = self.env.enum_variants(enum_fqn)
            {
                // 泛型 enum variant 在无期望类型上下文时构造歧义（无法推断类型实参）。
                let enum_has_type_params = self
                    .env
                    .type_constraints(enum_fqn)
                    .is_some_and(|(tps, _)| !tps.is_empty());
                if enum_has_type_params {
                    self.diags
                        .push(diagnostics::ambiguous_enum_variant_ctor(span));
                }
                // 找到 variant 名，检查 arity。
                let var_name = self
                    .env
                    .interner
                    .resolve(fqn)
                    .rsplit('.')
                    .next()
                    .unwrap_or("");
                if variants
                    .iter()
                    .any(|&v| self.env.interner.resolve(v) == var_name)
                {
                    // 这是 enum variant ctor。
                    // 从 Index 查 variant 的 payload 字段数（当前无法直接获取 → 用 Index 的 ctor_params）。
                    // 尝试用 enum_fqn 的 ctor_params（但 enum variant 字段不在 enum 的 ctor 中）。
                    // 保守：跳过 arity 检查（需要 variant payload 注册）。
                }
            }
        }
        // enum variant ctor arity 检查：Option 的 Some 有 1 个 payload。
        // 特殊处理 prelude enum（Option/Result）——它们的 variant ctor 参数数量已知。
        let known_arity = self.known_enum_variant_arity(fqn);
        if let Some(expected_arity) = known_arity
            && expected_arity != args.len()
            && !args.iter().any(|a| a.name.is_some())
        {
            if !self.lenient_type_errors {
                self.diags
                    .push(diagnostics::enum_variant_ctor_arity_mismatch(
                        expected_arity,
                        args.len(),
                        span,
                    ));
            }
            for a in args {
                self.walk_expr(&a.value);
            }
            let nominal = NominalType {
                fqn,
                args: vec![],
                eff: None,
            };
            return if self.env.is_reference_nominal(fqn) {
                self.env.store.ref_nominal(nominal)
            } else {
                self.env.store.value_nominal(nominal)
            };
        }
        let nominal = NominalType {
            fqn,
            args: vec![],
            eff: None,
        };
        let result = if self.env.is_reference_nominal(fqn) {
            self.env.store.ref_nominal(nominal)
        } else {
            self.env.store.value_nominal(nominal)
        };
        // struct/class 命名实参校验：命名实参名必须是已声明的字段/属性。
        let field_names: Option<std::collections::HashSet<Symbol>> = self
            .env
            .member_types(fqn)
            .filter(|m| !m.is_empty())
            .map(|m| m.keys().copied().collect());
        if let Some(names) = field_names {
            for a in args {
                if let Some(arg_name) = &a.name
                    && !names.contains(&arg_name.symbol)
                {
                    let n = self.env.interner.resolve(arg_name.symbol);
                    self.diags
                        .push(diagnostics::unknown_call_arg_name(n, a.span));
                }
            }
        }
        // 显式类型实参冲突检测（`Box<Int>("hi")`：T=Int 但实参是 String）。
        // 若显式实参个数 == 构造器泛型形参个数，且某位置实参类型不可赋值给对应显式类型实参，
        // 报 no_applicable_overload（generic type arguments 冲突）。
        if !explicit_arg_types.is_empty()
            && let Some((_, constraints)) = self.env.type_constraints(fqn)
        {
            // 仅当显式实参个数匹配类型形参个数时检测。
            let tp_count = self
                .env
                .type_constraints(fqn)
                .map(|(tps, _)| tps.len())
                .unwrap_or(0);
            if explicit_arg_types.len() == tp_count && tp_count > 0 {
                let _ = constraints;
                // 逐位置实参（位置实参）与显式类型实参比对：
                // 构造器形参若含类型参数 T，对应位置的实参类型必须可赋值给 explicit_arg_types[T 的位置]。
                let mut conflict = false;
                for (i, p) in params.iter().enumerate() {
                    if let TypeKind::Param(tp) = self.env.store.kind(*p)
                        && let Some(arg_idx) = args.iter().position(|a| {
                            a.name.is_none() && {
                                // 第 i 个位置实参的下标（仅位置实参）。
                                let pos_count =
                                    args.iter().take_while(|a| a.name.is_none()).count();
                                i < pos_count
                            }
                        })
                    {
                        // 找到 T 在类型形参列表中的位置。
                        if let Some(tp_pos) = self
                            .env
                            .type_constraints(fqn)
                            .and_then(|(tps, _)| tps.iter().position(|&n| n == tp.name))
                        {
                            let arg_ty = self.walk_expr(&args[arg_idx].value);
                            let explicit = explicit_arg_types.get(tp_pos).copied();
                            if let Some(explicit) = explicit
                                && !self.assignable(arg_ty, explicit)
                            {
                                conflict = true;
                                break;
                            }
                        }
                    }
                }
                if conflict {
                    // 添加类型声明处的 related 标注（帮助定位冲突的泛型参数声明）。
                    let decl_span = self
                        .env
                        .index
                        .lookup_type(fqn)
                        .map(|d| d.span)
                        .unwrap_or_default();
                    let mut diag = diagnostics::no_applicable_overload_with_reason(
                        "generic type arguments 与 ctor 实参反推的类型实参冲突",
                        span,
                    );
                    if decl_span.start != 0 || decl_span.end != 0 {
                        let name = self.env.interner.resolve(fqn).to_string();
                        diag = diag.with_related(decl_span, name);
                    }
                    self.diags.push(diag);
                }
            }
        }
        // 应用显式类型实参到返回 nominal。
        let result = if explicit_arg_types.is_empty() {
            result
        } else {
            let nominal = NominalType {
                fqn,
                args: explicit_arg_types.clone(),
                eff: None,
            };
            if self.env.is_reference_nominal(fqn) {
                self.env.store.ref_nominal(nominal)
            } else {
                self.env.store.value_nominal(nominal)
            }
        };
        self.check_call_args(params, result, args, span)
    }

    /// spread 实参 `*expr` 类型校验：必须是 Array / tuple；MutableArray/MutableList 等集合建议 toArray()。
    fn check_spread_arg(&mut self, ty: TypeId, span: Span) {
        let (ok, suggest) = {
            let kind = self.env.store.kind(ty);
            let nominal = nominal_fqn_of(kind).map(|fqn| {
                let n = self.env.interner.resolve(fqn);
                n.strip_prefix("scoop.core.").unwrap_or(n).to_string()
            });
            let is_array = nominal.as_deref() == Some("Array");
            let is_tuple = matches!(kind, TypeKind::Value(crate::ty::ValueTypeKind::Tuple(_)));
            // MutableArray / MutableList / List 等集合需要 toArray() 桥接，不能直接 spread。
            let suggest = matches!(
                nominal.as_deref(),
                Some("MutableArray" | "MutableList" | "List")
            );
            (is_array || is_tuple, suggest)
        };
        if !ok {
            self.diags
                .push(diagnostics::vararg_spread_requires_array_or_tuple(
                    span, suggest,
                ));
        }
    }

    /// 类型是否实现 `ToString`（标量原始类型 + nominal 超类型链中含 ToString）。
    fn type_implements_tostring(&self, ty: TypeId) -> bool {
        let kind = self.env.store.kind(ty);
        let fqn = nominal_fqn_of(kind).or_else(|| scalar_fqn(kind, self.env.interner));
        let Some(fqn) = fqn else {
            // Nothing（不可解析）放行；其余无 FQN 类型放行。
            return self.env.store.is_nothing(ty);
        };
        let target = self
            .env
            .interner
            .get("scoop.core.ToString")
            .or_else(|| self.env.interner.get("ToString"));
        let Some(target) = target else {
            return true;
        };
        let mut visited: Vec<scoop2_base::Symbol> = Vec::new();
        let mut stack = vec![fqn];
        while let Some(f) = stack.pop() {
            if f == target {
                return true;
            }
            if visited.contains(&f) {
                continue;
            }
            visited.push(f);
            for &sup in self.env.index.supertypes_of(f) {
                stack.push(sup);
            }
        }
        false
    }

    /// 名称是否是已知 effect 类型（prelude + 用户声明）。
    fn is_effect_type_name(&self, name: &str) -> bool {
        // prelude effects.
        let stripped = name.strip_prefix("scoop.core.").unwrap_or(name);
        if matches!(stripped, "Raise" | "RuntimeError") {
            return true;
        }
        // 用户声明的 effect 按 FQN 注册（如 `fixtures.typecheck.Ask`）；
        // 裸名需尝试 package prefix + scoop.core 候选。
        for candidate in [
            name.to_string(),
            format!("{}.{}", self.package_prefix, name),
            format!("scoop.core.{}", name),
        ] {
            if let Some(sym) = self.env.interner.get(&candidate)
                && let Some(cat) = self.env.index.category(sym)
                && matches!(cat, crate::resolve::symbol::NominalCategory::Effect)
            {
                return true;
            }
        }
        false
    }

    /// 将 effect 类型名解析为 nominal TypeId（用于 lambda body effect 构造 EffectRow）。
    fn effect_type_to_typeid(&mut self, name: &str) -> Option<TypeId> {
        // 候选 FQN：优先 package prefix（用户 effect），再 scoop.core（prelude），再裸名。
        // 避免裸名 `IO` 命中 interner 中残留的非 FQN 符号而非 `fixtures.typecheck.IO`。
        let prefix = self.package_prefix.clone();
        let candidates = [
            format!("{}.{}", prefix, name),
            format!("scoop.core.{}", name),
            name.to_string(),
        ];
        for candidate in &candidates {
            if let Some(sym) = self.env.interner.get(candidate) {
                // 仅接受在 Index 中注册为类型（含 Effect category）的 FQN。
                if self.env.index.category(sym).is_none() {
                    continue;
                }
                let is_ref = self.env.is_reference_nominal(sym);
                let nominal = NominalType {
                    fqn: sym,
                    args: vec![],
                    eff: None,
                };
                return Some(if is_ref {
                    self.env.store.ref_nominal(nominal)
                } else {
                    self.env.store.value_nominal(nominal)
                });
            }
        }
        None
    }

    /// 记录被调用函数声明的 effect 到 performed_effects（effect 传播）。
    /// 在 handle/lambda 体内不记录（被捕获）。
    /// `<eff E = Pure>` 泛型函数的嵌套 effect row 参数检查：
    /// 从第 1 个参数的 effect row 推断 E 包含的 effect 名，然后用 E 的推断值替换
    /// 后续参数类型中引用 E 的 effect row（`E + IO` → `inferred_effects + IO`），
    /// 再做逐位类型检查。
    fn check_eff_param_args(
        &mut self,
        sig: &Signature,
        arg_types: &[TypeId],
        args: &[CallArg],
        span: Span,
    ) {
        // 收集函数声明的 effect row 变量名（如 `E`）。
        let eff_var_name = self.eff_var_of_fqn_or_sig(sig);
        let Some(eff_var) = eff_var_name else {
            return;
        };
        // 从第 1 个参数的函数类型 effect row 推断 E 的 effect 集。
        // `() -> Unit / (Raise<Int> + IO)` → E 推断为 `{Raise<Int>}`（减去签名中的 `IO` 等）。
        let inferred_effs = self.infer_eff_var_from_arg(&eff_var, sig, arg_types);
        if inferred_effs.is_empty() {
            return;
        }
        // 对第 2+ 个参数：将 E 替换为推断值后做类型检查。
        for (i, (&at, &pt)) in arg_types.iter().zip(&sig.params).enumerate().skip(1) {
            let substituted_pt = self.subst_eff_var_in_type(pt, &eff_var, &inferred_effs);
            if substituted_pt != pt
                && !self.assignable(at, substituted_pt)
                && !self.lenient_type_errors
            {
                self.diags.push(diagnostics::call_arg_type_mismatch(
                    &self.describe(substituted_pt),
                    &self.describe(at),
                    args.get(i).map(|a| a.value.span).unwrap_or(span),
                ));
            }
        }
    }

    /// 从签名查找 `<eff E = Pure>` 的 E 变量名。
    fn eff_var_of_fqn_or_sig(&self, _sig: &Signature) -> Option<scoop2_base::Symbol> {
        // 签名不直接携带 eff 变量名；我们用约定的 `E` 名（spec §5.4: eff 参数名）。
        // 这是一个简化——若有多 eff 变量或不同名，需从 Index 查。
        self.env.interner.get("E")
    }

    /// 从第 1 个参数的函数类型 effect row 推断 E 的 effect 集。
    /// 签名第 1 参数：`() -> Unit / E`。
    /// 实参第 1 参数：`() -> Unit / (Raise<Int> + IO)`。
    /// 推断：E = `Raise<Int>`（实参 effect row 减去签名中不依赖 E 的 effect）。
    fn infer_eff_var_from_arg(
        &self,
        _eff_var: &scoop2_base::Symbol,
        sig: &Signature,
        arg_types: &[TypeId],
    ) -> Vec<TypeId> {
        let Some(&sig_pt) = sig.params.first() else {
            return vec![];
        };
        let Some(&arg_pt) = arg_types.first() else {
            return vec![];
        };
        // 提取签名第 1 参数的 effect row。
        let sig_effs = match self.env.store.kind(sig_pt) {
            TypeKind::Ref(crate::ty::RefTypeKind::Function(f)) => f.effects.terms.clone(),
            _ => return vec![],
        };
        // 提取实参第 1 参数的 effect row。
        let arg_effs = match self.env.store.kind(arg_pt) {
            TypeKind::Ref(crate::ty::RefTypeKind::Function(f)) => f.effects.terms.clone(),
            _ => return vec![],
        };
        // 实参 effect row 中不在签名 effect row 里的项 = E 推断值。
        arg_effs
            .into_iter()
            .filter(|ae| !sig_effs.iter().any(|se| se == ae))
            .collect()
    }

    /// 在类型 `ty` 中将 effect row 变量 `eff_var` 替换为 `inferred` effect 集。
    /// 递归处理函数类型（参数、返回值、effect row 中的 `E + IO`）。
    /// 在类型 `ty` 中将所有 `TypeKind::Param` 替换为 `subst`。
    fn subst_all_params(&mut self, ty: TypeId, subst: TypeId) -> TypeId {
        match self.env.store.kind(ty).clone() {
            TypeKind::Param(_) => subst,
            TypeKind::Ref(crate::ty::RefTypeKind::Function(ft)) => {
                let params: Vec<TypeId> = ft
                    .params
                    .iter()
                    .map(|&p| self.subst_all_params(p, subst))
                    .collect();
                let return_ty = self.subst_all_params(ft.return_ty, subst);
                let receiver = ft.receiver.map(|r| self.subst_all_params(r, subst));
                self.env.store.function(FunctionType {
                    receiver,
                    params,
                    return_ty,
                    effects: ft.effects,
                    closed: ft.closed,
                })
            }
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => {
                let args: Vec<TypeId> = n
                    .args
                    .iter()
                    .map(|&a| self.subst_all_params(a, subst))
                    .collect();
                self.env.store.ref_nominal(crate::ty::NominalType {
                    fqn: n.fqn,
                    args,
                    eff: n.eff,
                })
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
                let args: Vec<TypeId> = n
                    .args
                    .iter()
                    .map(|&a| self.subst_all_params(a, subst))
                    .collect();
                self.env.store.value_nominal(crate::ty::NominalType {
                    fqn: n.fqn,
                    args,
                    eff: n.eff,
                })
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Tuple(els)) => {
                let new_els: Vec<TypeId> = els
                    .iter()
                    .map(|&e| self.subst_all_params(e, subst))
                    .collect();
                self.env.store.tuple(new_els)
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Option(inner)) => {
                let new_inner = self.subst_all_params(inner, subst);
                self.env.store.option(new_inner)
            }
            _ => ty,
        }
    }

    fn subst_eff_var_in_type(
        &mut self,
        ty: TypeId,
        eff_var: &scoop2_base::Symbol,
        inferred: &[TypeId],
    ) -> TypeId {
        let kind = self.env.store.kind(ty).clone();
        match kind {
            TypeKind::Ref(crate::ty::RefTypeKind::Function(ft)) => {
                let params: Vec<TypeId> = ft
                    .params
                    .iter()
                    .map(|&p| self.subst_eff_var_in_type(p, eff_var, inferred))
                    .collect();
                let return_ty = self.subst_eff_var_in_type(ft.return_ty, eff_var, inferred);
                let new_effects = self.subst_eff_row(&ft.effects, eff_var, inferred);
                let new_ft = FunctionType {
                    receiver: ft.receiver,
                    params,
                    return_ty,
                    effects: new_effects,
                    closed: ft.closed,
                };
                self.env.store.function(new_ft)
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Tuple(els)) => {
                let new_els: Vec<TypeId> = els
                    .iter()
                    .map(|&e| self.subst_eff_var_in_type(e, eff_var, inferred))
                    .collect();
                self.env.store.tuple(new_els)
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Option(inner)) => {
                let new_inner = self.subst_eff_var_in_type(inner, eff_var, inferred);
                self.env.store.option(new_inner)
            }
            _ => ty,
        }
    }

    /// 在 effect row 中将变量 `eff_var` 替换为 `inferred` effect 集。
    /// `E + IO` → `inferred + IO`。
    fn subst_eff_row(
        &self,
        row: &crate::ty::EffectRow,
        eff_var: &scoop2_base::Symbol,
        inferred: &[TypeId],
    ) -> crate::ty::EffectRow {
        let mut new_terms: Vec<TypeId> = Vec::new();
        for &term in &row.terms {
            if let TypeKind::Param(p) = self.env.store.kind(term)
                && &p.name == eff_var
            {
                new_terms.extend_from_slice(inferred);
                continue;
            }
            new_terms.push(term);
        }
        crate::ty::EffectRow::from_terms(new_terms)
    }

    /// 泛型 `<eff E>` 函数：从 lambda 实参体推断 E 的 effect 集，记录之。
    fn record_callee_effects(&mut self, sig: &Signature, args: &[CallArg], span: Span) {
        if self.effect_suspend_depth > 0 {
            return;
        }
        let all_names = extract_effect_row_names(sig.effect.as_ref(), self.env.interner);
        let names: Vec<String> = all_names
            .iter()
            .filter(|name| self.is_effect_type_name(name))
            .cloned()
            .collect();
        // 泛型 `<eff E>` 推断：effect 行引用 E（eff 变量）但无具体 effect 名时，
        // 从 lambda 实参体推断 E 的值。
        let has_eff_var = all_names
            .iter()
            .any(|n| !self.is_effect_type_name(n) && n != "Pure");
        let inferred: Vec<String> = if has_eff_var {
            let mut effects = self.infer_eff_from_lambda_args(args);
            // Nominal eff arg 推断：实参类型携带 use-site eff 实参（`Disposable<eff Async>`）。
            for arg in args {
                let arg_ty = self.walk_expr(&arg.value);
                if nominal_fqn_of(self.env.store.kind(arg_ty)).is_some() {
                    let nominal_eff = match self.env.store.kind(arg_ty) {
                        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nm))
                        | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nm)) => nm.eff.as_ref(),
                        _ => None,
                    };
                    if let Some(eff_row) = nominal_eff {
                        for term_ty in &eff_row.terms {
                            if let Some(fqn) = nominal_fqn_of(self.env.store.kind(*term_ty)) {
                                let name = self.env.interner.resolve(fqn);
                                let s =
                                    name.strip_prefix("scoop.core.").unwrap_or(name).to_string();
                                if self.is_effect_type_name(&s) && !effects.iter().any(|e| e == &s)
                                {
                                    effects.push(s);
                                }
                            }
                        }
                    }
                }
            }
            effects
        } else {
            Vec::new()
        };
        for name in names.into_iter().chain(inferred) {
            self.performed_effects.push((name, span));
        }
    }

    /// 从 lambda 实参体中扫描 effect 操作（AST 级扫描，不触发诊断）。
    fn infer_eff_from_lambda_args(&self, args: &[CallArg]) -> Vec<String> {
        let mut effects: Vec<String> = Vec::new();
        for arg in args {
            // Lambda 实参：扫描 body 中的 effect 操作。
            if let ast::ExprKind::Lambda(lambda) = &arg.value.kind {
                match &lambda.body {
                    ast::LambdaBody::Block(b) => {
                        self.scan_block_for_effect_ops(b, &mut effects);
                    }
                    ast::LambdaBody::Expr(e) => {
                        self.scan_expr_for_effect_ops(e, &mut effects);
                    }
                }
            }
            // 非 lambda 实参：也可能是内联 effect 操作（如 `use(Raise.raise(1))`）。
            self.scan_expr_for_effect_ops(&arg.value, &mut effects);
        }
        effects
    }

    fn scan_block_for_effect_ops(&self, block: &ast::Block, effects: &mut Vec<String>) {
        for s in &block.stmts {
            match &s.kind {
                ast::StmtKind::Expr(e) => self.scan_expr_for_effect_ops(e, effects),
                ast::StmtKind::Assign { value, .. } => {
                    self.scan_expr_for_effect_ops(value, effects)
                }
                ast::StmtKind::Return { value: Some(e) } => {
                    self.scan_expr_for_effect_ops(e, effects)
                }
                _ => {}
            }
        }
    }

    fn scan_expr_for_effect_ops(&self, expr: &ast::Expr, effects: &mut Vec<String>) {
        use crate::syntax::ast::ExprKind;
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                // EffectName.op(...) → 记录 EffectName。
                if let ExprKind::MemberAccess { receiver, .. } = &callee.kind
                    && let ExprKind::Ident(id) = &receiver.kind
                {
                    let name = self.env.interner.resolve(id.symbol);
                    let stripped = name.strip_prefix("scoop.core.").unwrap_or(name);
                    if self.is_effect_type_name(stripped) && !effects.iter().any(|e| e == stripped)
                    {
                        effects.push(stripped.to_string());
                    }
                }
                self.scan_expr_for_effect_ops(callee, effects);
                for a in args {
                    self.scan_expr_for_effect_ops(&a.value, effects);
                }
            }
            ExprKind::Block(b)
            | ExprKind::DoBlock(b)
            | ExprKind::UnsafeBlock(b)
            | ExprKind::SafeBlock(b) => {
                self.scan_block_for_effect_ops(b, effects);
            }
            ExprKind::Lambda(_) => {
                // 嵌套 lambda 的 effect 由该 lambda 自己的 `<eff E>` 捕获，不传播。
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.scan_expr_for_effect_ops(cond, effects);
                self.scan_expr_for_effect_ops(then_branch, effects);
                if let Some(e) = else_branch {
                    self.scan_expr_for_effect_ops(e, effects);
                }
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.scan_expr_for_effect_ops(lhs, effects);
                self.scan_expr_for_effect_ops(rhs, effects);
            }
            ExprKind::Unary { expr: e, .. } => {
                self.scan_expr_for_effect_ops(e, effects);
            }
            ExprKind::MemberAccess { receiver, .. } => {
                self.scan_expr_for_effect_ops(receiver, effects);
            }
            ExprKind::Index { receiver, indices } => {
                self.scan_expr_for_effect_ops(receiver, effects);
                for i in indices {
                    self.scan_expr_for_effect_ops(i, effects);
                }
            }
            ExprKind::TupleLit(els) | ExprKind::ArrayLit(els) => {
                for e in els {
                    self.scan_expr_for_effect_ops(e, effects);
                }
            }
            ExprKind::NotNullAssert { expr: e } => {
                self.scan_expr_for_effect_ops(e, effects);
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for p in parts {
                    if let ast::StringPart::Expr(e) = p {
                        self.scan_expr_for_effect_ops(e, effects);
                    }
                }
            }
            _ => {}
        }
    }

    /// 记录 `k.resume(v)` 的步骤 effect 到 performed_effects。
    ///
    /// `Continuation<Resume, Answer, eff E>.resume` 的 effect 行为 `E + Raise<RuntimeError>`
    /// （spec §5.5）：恒定执行 `Raise`（即 `Raise<RuntimeError>`），外加 E 解析出的具体 effect。
    /// E 为类型参数（多态）或 `Pure` 时不贡献额外 effect。
    /// 当 receiver 类型未知（Nothing，如 pattern binder）时仍记录恒定 `Raise`。
    fn record_continuation_resume_effects(&mut self, recv_ty: TypeId, span: Span) {
        let kind = self.env.store.kind(recv_ty);
        // Continuation 或未知（Nothing/binder）→ resume 恒定执行 Raise<RuntimeError>。
        // 已知非 Continuation receiver → 不记录。
        let is_continuation_or_unknown = match nominal_fqn_of(kind) {
            Some(fqn) => {
                let n = self.env.interner.resolve(fqn);
                n.strip_prefix("scoop.core.").unwrap_or(n) == "Continuation"
            }
            None => true,
        };
        if !is_continuation_or_unknown {
            return;
        }
        // 在 effect_suspend_depth > 0（handle arm body 内）时，不记录 Raise（已被 handle 捕获），
        // 仅记录 resumed-step effect（非 Pure / 非 Raise）。
        if self.effect_suspend_depth == 0 {
            self.performed_effects.push(("Raise".to_string(), span));
        }
        let kind_clone = kind.clone();
        self.record_resume_step_effects(&kind_clone, span);
    }

    /// 仅记录 Continuation 的 resumed-step effect（非 Pure / 非 Raise）。
    fn record_resume_step_effects(&mut self, kind: &TypeKind, span: Span) {
        // resumed-step effect 来自 continuation 的 eff row（build_continuation_type_with_effects
        // 从 handle body 提取，已排除被 handle 捕获的 effect）。无论 effect_suspend_depth 如何，
        // resume 执行时这些未捕获的 effect 都会发生，必须穿透到外层函数。
        // 记录到 escape_effects（不受 handle arm 体截断影响）。
        // 从 nominal args[2] 提取（旧式 Continuation<Resume, Answer, eff E>）。
        if let Some(args) = nominal_args_of(kind)
            && let Some(&e_ty) = args.get(2)
            && let Some(e_fqn) = nominal_fqn_of(self.env.store.kind(e_ty))
        {
            let n = self.env.interner.resolve(e_fqn).to_string();
            let stripped = n.strip_prefix("scoop.core.").unwrap_or(&n);
            if stripped != "Pure" && stripped != "Raise" {
                self.escape_effects.push((stripped.to_string(), span));
            }
        }
        // 从 nominal 的 eff 字段提取（新式 Continuation<Resume, Answer, eff {A + B}>）。
        if let Some(n) = match kind {
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(n),
            _ => None,
        } && let Some(eff_row) = &n.eff
        {
            for &term_ty in &eff_row.terms {
                if let Some(e_fqn) = nominal_fqn_of(self.env.store.kind(term_ty)) {
                    let n = self.env.interner.resolve(e_fqn).to_string();
                    let stripped = n.strip_prefix("scoop.core.").unwrap_or(&n);
                    if stripped != "Pure" && stripped != "Raise" {
                        self.escape_effects.push((stripped.to_string(), span));
                    }
                }
            }
        }
    }

    /// 解析 effect operation 的 Resume 类型（操作返回类型）。
    /// `Ask.current()` → 查 `Ask` effect 的 `current` 成员签名返回类型。
    fn effect_op_resume_type(&self, op: &ast::HandleOp) -> Option<TypeId> {
        let effect_fqn_text: String = op
            .effect_path
            .segments
            .iter()
            .map(|s| self.env.interner.resolve(s.symbol))
            .collect::<Vec<_>>()
            .join(".");
        let effect_fqn = self.env.interner.get(&effect_fqn_text)?;
        let sigs = self.env.member_signatures(effect_fqn, op.op.symbol)?;
        let sig = sigs.first()?;
        Some(sig.return_ty)
    }

    /// 构造 `Continuation<Resume, Answer, eff Pure>` 类型。
    #[allow(dead_code)]
    fn build_continuation_type(&mut self, resume_ty: TypeId, answer_ty: TypeId) -> TypeId {
        self.build_continuation_type_with_effects(resume_ty, answer_ty, &[])
    }

    /// 构造 `Continuation<Resume, Answer, eff Row>` 类型，Row 由 handle 体执行的 effect 组成。
    /// `body_effects` 为 effect 短名（如 `Boom`）；空则 Row = Pure。
    fn build_continuation_type_with_effects(
        &mut self,
        resume_ty: TypeId,
        answer_ty: TypeId,
        body_effects: &[String],
    ) -> TypeId {
        let cont_fqn = self
            .env
            .interner
            .get("scoop.core.Continuation")
            .or_else(|| self.env.interner.get("Continuation"));
        let Some(fqn) = cont_fqn else {
            return self.env.store.nothing();
        };
        // 构造 effect row：每个 effect 短名 → nominal 引用。
        // 候选 FQN：裸名 / package prefix / scoop.core（用户 effect 按 package FQN 注册）。
        let mut terms: Vec<TypeId> = Vec::new();
        for eff_name in body_effects {
            let candidates = [
                eff_name.clone(),
                format!("{}.{}", self.package_prefix, eff_name),
                format!("scoop.core.{eff_name}"),
            ];
            for cand in &candidates {
                if let Some(eff_fqn) = self.env.interner.get(cand) {
                    let nominal = NominalType {
                        fqn: eff_fqn,
                        args: vec![],
                        eff: None,
                    };
                    terms.push(self.env.store.ref_nominal(nominal));
                    break;
                }
            }
        }
        let eff_row = if terms.is_empty() {
            crate::ty::EffectRow::pure()
        } else {
            crate::ty::EffectRow::from_terms(terms)
        };
        let nominal = NominalType {
            fqn,
            args: vec![resume_ty, answer_ty],
            eff: Some(eff_row),
        };
        self.env.store.ref_nominal(nominal)
    }

    /// 从 EffectRowExpr 提取 effect FQN 短名集合（不降级到 TypeId）。
    fn extract_effect_names(
        eff: Option<&ast::EffectRowExpr>,
        interner: &scoop2_base::Interner,
    ) -> HashSet<String> {
        let mut names = HashSet::new();
        if let Some(eff) = eff {
            for term in &eff.terms {
                if let Some(seg) = term.path.segments.last() {
                    let name = interner.resolve(seg.symbol);
                    if name != "Pure" {
                        names.insert(name.to_string());
                    }
                }
            }
        }
        names
    }

    /// 已知 prelude enum variant 的 payload 数量。
    fn known_enum_variant_arity(&self, fqn: Symbol) -> Option<usize> {
        let name = self.env.interner.resolve(fqn);
        // Option variants.
        if name.ends_with(".Some") || name == "Some" {
            return Some(1);
        }
        if name.ends_with(".None") || name == "None" {
            return Some(0);
        }
        None
    }

    /// variant payload arity：结合内建 Option 特判与用户 enum 的登记表。
    /// `enum_fqn` 为 subject 的 enum 全限定名；`variant` 为 variant 名符号。
    fn variant_payload_arity(&self, enum_fqn: Symbol, variant: Symbol) -> Option<usize> {
        let variant_name = self.env.interner.resolve(variant);
        // 内建 Option：Some=1, None=0（subject 是 Option<T>，enum_fqn 末段为 Option）。
        let enum_name = self.env.interner.resolve(enum_fqn);
        if enum_name.ends_with(".Option") || enum_name == "Option" {
            if variant_name == "Some" {
                return Some(1);
            }
            if variant_name == "None" {
                return Some(0);
            }
        }
        // 用户 enum：(enum FQN, variant 名) → payload 字段数。
        self.env.enum_variant_arity(enum_fqn, variant)
    }

    /// 构造器签名对给定实参是否适用（位置实参 arity ∈ [min_arity, n] + 类型；命名实参映射）。
    fn ctor_sig_applicable(&self, s: &Signature, args: &[CallArg], arg_types: &[TypeId]) -> bool {
        let n = s.params.len();
        // vararg 签名：最后一个参数可选（可传 0 个），min_arity = n-1（若无默认值）。
        let mut min_arity = s.has_defaults.iter().position(|d| *d).unwrap_or(n);
        if s.has_vararg && n > 0 {
            min_arity = min_arity.min(n - 1);
        }
        let has_named = args.iter().any(|a| a.name.is_some());
        if has_named {
            // 命名实参：每个名字必须在签名中，且类型可赋值；未提供的必需参数须有默认。
            for (i, &pname) in s.param_names.iter().enumerate() {
                let has_default = s.has_defaults.get(i).copied().unwrap_or(false);
                let arg_idx = args
                    .iter()
                    .position(|a| a.name.as_ref().is_some_and(|n| n.symbol == pname));
                if arg_idx.is_none() && !has_default {
                    return false;
                }
                if let Some(idx) = arg_idx
                    && !self.assignable(arg_types[idx], s.params[i])
                {
                    return false;
                }
            }
            true
        } else {
            // 位置实参：arity ∈ [min_arity, n]，逐位类型可赋值。
            // vararg 签名（最后一个参数为 vararg）：允许任意多余位置实参（arity ≥ min_arity）。
            if arg_types.len() < min_arity || (!s.has_vararg && arg_types.len() > n) {
                return false;
            }
            if s.type_param_count > 0 {
                let bound = s.type_param_bounds.first().and_then(|b| *b);
                arg_types
                    .iter()
                    .all(|a| bound.is_none_or(|bt| self.assignable(*a, bt)))
            } else {
                arg_types
                    .iter()
                    .zip(&s.params)
                    .all(|(a, p)| self.assignable(*a, *p))
            }
        }
    }

    /// 次构造器重载决议：适用性 + 特异性（concrete 优于 generic；bound 不可比则歧义）。
    fn resolve_ctor_overloads(
        &mut self,
        fqn: Symbol,
        ctors: &[Signature],
        args: &[CallArg],
        span: Span,
        explicit_type_args: &[crate::syntax::ast::TypeArg],
    ) -> TypeId {
        // 显式类型实参降级（`Container<String>()`）。
        let explicit_arg_types: Vec<TypeId> = explicit_type_args
            .iter()
            .filter_map(|a| match &a.kind {
                crate::syntax::ast::TypeArgKind::Type(t) => Some(self.lower_type(t)),
                _ => None,
            })
            .collect();
        let nominal = NominalType {
            fqn,
            args: explicit_arg_types.clone(),
            eff: None,
        };
        let result = if self.env.is_reference_nominal(fqn) {
            self.env.store.ref_nominal(nominal)
        } else {
            self.env.store.value_nominal(nominal)
        };
        let arg_types: Vec<TypeId> = args.iter().map(|a| self.walk_expr(&a.value)).collect();
        let applicable: Vec<&Signature> = ctors
            .iter()
            .filter(|s| self.ctor_sig_applicable(s, args, &arg_types))
            .collect();
        if applicable.is_empty() {
            // 优先检测未知命名实参（给出更精确诊断）：若某命名实参名不在任何 ctor 的 param_names 中，
            // 报 unknown_call_arg_name（而非笼统的 no_applicable_overload）。
            let has_named = args.iter().any(|a| a.name.is_some());
            if has_named {
                for a in args {
                    if let Some(arg_name) = &a.name {
                        let known = ctors
                            .iter()
                            .any(|s| s.param_names.contains(&arg_name.symbol));
                        if !known {
                            let n = self.env.interner.resolve(arg_name.symbol);
                            self.diags
                                .push(diagnostics::unknown_call_arg_name(n, a.span));
                            return result;
                        }
                    }
                }
            }
            // 构建详细诊断消息：列出候选 + 不匹配原因。
            let fqn_text = self.env.interner.resolve(fqn).to_string();
            let fun_name = fqn_text.rsplit('.').next().unwrap_or(&fqn_text);
            let arg_types: Vec<TypeId> = args.iter().map(|a| self.walk_expr(&a.value)).collect();
            let mut msg = "没有匹配的重载候选".to_string();
            for sig in ctors.iter() {
                let params: Vec<String> = sig
                    .params
                    .iter()
                    .map(|p| self.describe(*p))
                    .collect::<Vec<_>>();
                msg.push_str(&format!("\n  {fun_name}({})", params.join(", ")));
                // 逐位类型检查（包括 args < params 的情况——默认值使 args 可以更少）。
                for (j, (at, pt)) in arg_types.iter().zip(&sig.params).enumerate() {
                    if !self.assignable(*at, *pt) {
                        let pname = sig
                            .param_names
                            .get(j)
                            .map(|s| self.env.interner.resolve(*s))
                            .unwrap_or("");
                        msg.push_str(&format!(
                            "\n    argument {} type {} is not a subtype of parameter `{pname}` type {}",
                            j + 1,
                            self.describe(*at),
                            self.describe(*pt),
                        ));
                    }
                }
            }
            let mut diag = scoop2_base::diag::Diagnostic::error(
                "scoop::typecheck::no_applicable_overload",
                msg,
            )
            .with_primary(span, "这里");
            // 添加候选声明的 related 标签（供诊断策略断言 `EXPECT-ERROR: file:line:col`）。
            for sig in ctors.iter() {
                let params: Vec<String> = sig
                    .params
                    .iter()
                    .map(|p| self.describe(*p))
                    .collect::<Vec<_>>();
                diag =
                    diag.with_related(sig.decl_span, format!("{fun_name}({})", params.join(", ")));
            }
            self.diags.push(diag);
            return result;
        }
        // 特异性：a 比 b 更具体（a.params ⊆ b.params 严格，或并列时 type_param_count 更少）。
        let more_specific = |a: &Signature, b: &Signature| -> bool {
            if a.params.len() != b.params.len() {
                return false;
            }
            let a_sub_b = a
                .params
                .iter()
                .zip(&b.params)
                .all(|(ap, bp)| self.assignable_strict(*ap, *bp));
            let b_sub_a = b
                .params
                .iter()
                .zip(&a.params)
                .all(|(bp, ap)| self.assignable_strict(*bp, *ap));
            let has_param = |s: &Signature| {
                s.params
                    .iter()
                    .any(|p| matches!(self.env.store.kind(*p), TypeKind::Param(_)))
            };
            if a_sub_b && !b_sub_a {
                true
            } else if a_sub_b && b_sub_a {
                // 并列：concrete（参数无类型参数）优于 generic（参数含类型参数）；
                // 否则按类型参数个数少者优先。
                if has_param(a) != has_param(b) {
                    !has_param(a) && has_param(b)
                } else {
                    a.type_param_count < b.type_param_count
                }
            } else {
                false
            }
        };
        let mut winner = vec![true; applicable.len()];
        for i in 0..applicable.len() {
            for j in 0..applicable.len() {
                if i != j && more_specific(applicable[j], applicable[i]) {
                    winner[i] = false;
                }
            }
        }
        let winners: Vec<usize> = winner
            .iter()
            .enumerate()
            .filter(|(_, w)| **w)
            .map(|(i, _)| i)
            .collect();
        if winners.len() == 1 {
            return result;
        }
        // 歧义（含 incomparable bounds）。
        let mut msg = "重载决议歧义：构造器不可区分".to_string();
        if let (Some(i), Some(j)) = (winners.first().copied(), winners.get(1).copied())
            && applicable[i].type_param_count > 0
            && applicable[j].type_param_count > 0
        {
            let bound_of = |s: &Signature| s.type_param_bounds.first().and_then(|b| *b);
            if let (Some(bi), Some(bj)) = (bound_of(applicable[i]), bound_of(applicable[j])) {
                let fqn_text = |t: TypeId| {
                    nominal_fqn_of(self.env.store.kind(t))
                        .map(|f| self.env.interner.resolve(f).to_string())
                        .unwrap_or_else(|| self.describe(t))
                };
                msg = format!(
                    "reason: {} (from `T` declared bound) and {} (from `T` declared bound) are incomparable",
                    fqn_text(bi),
                    fqn_text(bj)
                );
            }
        }
        let mut diag =
            scoop2_base::diag::Diagnostic::error("scoop::typecheck::ambiguous_overload", msg)
                .with_primary(span, "这里");
        let fqn_text = self.env.interner.resolve(fqn).to_string();
        let name = fqn_text.rsplit('.').next().unwrap_or(&fqn_text);
        for &w in &winners {
            let params: Vec<String> = applicable[w]
                .params
                .iter()
                .map(|p| self.describe(*p))
                .collect();
            diag = diag.with_related(
                applicable[w].decl_span,
                format!("{name}({})", params.join(", ")),
            );
        }
        let _ = &mut diag; // quiet
        self.diags.push(diag);
        result
    }

    /// 查找 enum variant FQN 的 enum owner FQN（`pkg.Enum.Variant` → `pkg.Enum`）。
    fn find_enum_owner(&self, variant_fqn: Symbol) -> Option<Symbol> {
        let name = self.env.interner.resolve(variant_fqn);
        if let Some(dot) = name.rfind('.') {
            let owner_text = &name[..dot];
            if let Some(owner) = self.env.interner.get(owner_text) {
                return Some(owner);
            }
        }
        // 裸名 variant：遍历所有 enum 的 variant 列表查找 owner。
        for (enum_fqn, variants) in &self.env.enum_variants {
            if variants.iter().any(|&v| {
                let vn = self.env.interner.resolve(v);
                vn == name || vn.rsplit('.').next() == Some(name)
            }) {
                return Some(*enum_fqn);
            }
        }
        None
    }

    /// 名字是否解析为类型符号（当前包 / import）；返回其 FQN。用于构造器调用判定。
    fn callee_type_fqn(&self, name: Symbol) -> Option<Symbol> {
        let name_text = self.env.interner.resolve(name);
        let fqn_text = if self.package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{}.{}", self.package_prefix, name_text)
        };
        if let Some(fqn) = self.env.interner.get(&fqn_text)
            && self.env.index.lookup_type(fqn).is_some()
        {
            return Some(fqn);
        }
        let fqn = self
            .imports
            .resolve_name(name, self.env.index, self.env.interner)?;
        if self.env.index.lookup_type(fqn).is_some() {
            Some(fqn)
        } else {
            None
        }
    }

    /// 顶层函数重载解析：单候选 → 直接检查（arity / 实参）；多候选 → 适用性过滤。
    fn resolve_top_level_call(&mut self, fqn: Symbol, args: &[CallArg], span: Span) -> TypeId {
        // scoop.thread.threadSpawn：线程入口必须在静态上等价于 Pure!。
        let callee_name = self.env.interner.resolve(fqn);
        let stripped = callee_name
            .strip_prefix("scoop.core.")
            .unwrap_or(callee_name);
        if (stripped == "threadSpawn" || callee_name.ends_with(".threadSpawn"))
            && let Some(first) = args.first()
        {
            let entry_ty = self.walk_expr(&first.value);
            let is_pure = matches!(
                self.env.store.kind(entry_ty),
                TypeKind::Ref(crate::ty::RefTypeKind::Function(f)) if f.effects.is_pure()
            );
            if !is_pure {
                self.diags
                    .push(diagnostics::thread_spawn_entry_must_be_pure(
                        &self.describe(entry_ty),
                        first.value.span,
                    ));
            }
        }
        // @NoGC 上下文：被调用函数必须是 @NoGC 或 native @Extern(c)。
        if self.in_nogc
            && let Some(attrs) = self.env.fun_attrs(fqn)
            && !attrs.is_nogc
            && !attrs.is_native_extern
        {
            let callee_name = self
                .env
                .interner
                .resolve(fqn)
                .rsplit('.')
                .next()
                .unwrap_or("");
            self.diags
                .push(diagnostics::nogc_call_forbidden(callee_name, span));
        }
        let sigs: Vec<Signature> = self.env.signatures(fqn).unwrap_or(&[]).to_vec();
        let non_generic: Vec<Signature> = sigs
            .iter()
            .filter(|s| s.type_param_count == 0)
            .cloned()
            .collect();
        if non_generic.is_empty() {
            // 泛型候选：按类型参数 bound 的适用性 + 特异性决议。
            let arg_types: Vec<TypeId> = args.iter().map(|a| self.walk_expr(&a.value)).collect();
            let decls = self.env.index.lookup_funs(fqn);
            // applicable = (sig 在 sigs 中的下标, Signature)，下标用于关联 decl。
            let mut applicable: Vec<(usize, Signature)> = Vec::new();
            for (idx, s) in sigs.iter().enumerate() {
                if s.type_param_count == 0 || s.params.len() != arg_types.len() {
                    continue;
                }
                // 有 bound：按 bound 适用性过滤。无 bound（如 `<eff E = Pure>`）：直接适用。
                let bound = s.type_param_bounds.first().and_then(|b| *b);
                let bound_ok =
                    bound.is_none_or(|bt| arg_types.iter().all(|a| self.assignable(*a, bt)));
                if bound_ok {
                    applicable.push((idx, s.clone()));
                }
            }
            if applicable.is_empty() {
                if let Some(sig) = sigs.first() {
                    self.check_generic_call_constraints(fqn, sig, &arg_types, span);
                }
                return self.env.store.nothing();
            }
            if applicable.len() < 2 {
                let (idx, sig) = applicable.into_iter().next().expect("non-empty");
                let _ = idx;
                self.check_generic_call_constraints(fqn, &sig, &arg_types, span);
                // <eff E = Pure> 泛型函数：从第 1 个参数推断 E 的 effect 集，
                // 然后将 E 替换到后续参数的嵌套 effect row 中做类型检查。
                self.check_eff_param_args(&sig, &arg_types, args, span);
                self.record_callee_effects(&sig, args, span);
                return sig.return_ty;
            }
            // 多候选：按 bound 特异性比较（bound1 <: bound2 → 更具体）。
            let bound_of = |s: &Signature| s.type_param_bounds.first().and_then(|b| *b);
            let mut winner = vec![true; applicable.len()];
            for i in 0..applicable.len() {
                for j in 0..applicable.len() {
                    if i == j {
                        continue;
                    }
                    let (Some(bi), Some(bj)) =
                        (bound_of(&applicable[i].1), bound_of(&applicable[j].1))
                    else {
                        continue;
                    };
                    // j 的 bound 比 i 的更具体（bj <: bi）且反向不成立 → i 出局。
                    if self.assignable_strict(bj, bi) && !self.assignable_strict(bi, bj) {
                        winner[i] = false;
                    }
                }
            }
            let winners: Vec<usize> = winner
                .iter()
                .enumerate()
                .filter(|(_, w)| **w)
                .map(|(i, _)| i)
                .collect();
            if winners.len() == 1 {
                let sig = &applicable[winners[0]].1;
                let ret = sig.return_ty;
                self.check_generic_call_constraints(fqn, sig, &arg_types, span);
                // <eff E = Pure> 泛型函数：从第 1 个参数推断 E 后替换到后续参数做类型检查。
                self.check_eff_param_args(sig, &arg_types, args, span);
                self.record_callee_effects(sig, args, span);
                return ret;
            }
            // 歧义：构造 incomparable 诊断 + related 标签。
            let mut msg = "重载决议歧义：泛型 bound 不可比较".to_string();
            if let (Some(i), Some(j)) = (winners.first().copied(), winners.get(1).copied())
                && let (Some(bi), Some(bj)) =
                    (bound_of(&applicable[i].1), bound_of(&applicable[j].1))
            {
                let fqn_text = |t: TypeId| {
                    nominal_fqn_of(self.env.store.kind(t))
                        .map(|f| self.env.interner.resolve(f).to_string())
                        .unwrap_or_else(|| self.describe(t))
                };
                msg = format!(
                    "reason: {} (from `T` declared bound) and {} (from `T` declared bound) are incomparable",
                    fqn_text(bi),
                    fqn_text(bj)
                );
            }
            let mut diag =
                scoop2_base::diag::Diagnostic::error("scoop::typecheck::ambiguous_overload", msg)
                    .with_primary(span, "这里");
            for &w in &winners {
                let (sig_idx, sig) = &applicable[w];
                let params: Vec<String> = sig.params.iter().map(|p| self.describe(*p)).collect();
                let label = format!("({})", params.join(", "));
                if let Some(d) = decls.get(*sig_idx) {
                    diag = diag.with_related(d.span, label);
                }
            }
            self.diags.push(diag);
            return applicable[0].1.return_ty;
        }
        // Unit sugar：0 实参 + 候选有 1 个 Unit 参数 → 补 Unit 实参（spec §5.5）。
        // 但若存在真正的 0 参候选，优先 exact-arity（不回退到 Unit sugar）。
        let unit_ty = self.env.store.unit();
        let arg_count = args.len();
        let has_zero_arity = non_generic.iter().any(|s| s.params.is_empty());
        if arg_count == 0
            && !has_zero_arity
            && non_generic
                .iter()
                .any(|s| s.params.len() == 1 && s.params[0] == unit_ty)
        {
            let sig = non_generic
                .into_iter()
                .find(|s| s.params.len() == 1 && s.params[0] == unit_ty)
                .expect("checked above");
            self.record_callee_effects(&sig, args, span);
            return sig.return_ty;
        }
        // 单候选：直接检查（保留 arity_mismatch / type_mismatch 精确诊断）。
        if non_generic.len() == 1 {
            let sig = non_generic.into_iter().next().expect("len == 1");
            // 命名实参校验：名字必须在该签名参数名中。
            let names: std::collections::HashSet<Symbol> =
                sig.param_names.iter().copied().collect();
            let mut any_unknown = false;
            for a in args {
                if let Some(arg_name) = &a.name
                    && !names.contains(&arg_name.symbol)
                {
                    any_unknown = true;
                    let n = self.env.interner.resolve(arg_name.symbol);
                    self.diags
                        .push(diagnostics::unknown_call_arg_name(n, a.span));
                }
            }
            // 命名实参不能跳过无默认值的必需参数。
            // 仅在命名实参结构合法（无未知名 / 无重名 / 无命名后位置）时报错，避免掩盖结构性诊断。
            let has_named = args.iter().any(|a| a.name.is_some());
            let mut seen_names: std::collections::HashSet<Symbol> =
                std::collections::HashSet::new();
            let has_dup = args.iter().any(|a| {
                a.name
                    .as_ref()
                    .is_some_and(|n| !seen_names.insert(n.symbol))
            });
            let mut seen_named = false;
            let pos_after_named = args.iter().any(|a| {
                if a.name.is_some() {
                    seen_named = true;
                    false
                } else {
                    seen_named
                }
            });
            if has_named && !any_unknown && !has_dup && !pos_after_named {
                let positional_count = args.iter().filter(|a| a.name.is_none()).count();
                let provided_names: std::collections::HashSet<Symbol> = args
                    .iter()
                    .filter_map(|a| a.name.as_ref().map(|n| n.symbol))
                    .collect();
                for (i, &pname) in sig.param_names.iter().enumerate() {
                    let has_default = sig.has_defaults.get(i).copied().unwrap_or(false);
                    if !has_default && i >= positional_count && !provided_names.contains(&pname) {
                        self.diags
                            .push(diagnostics::call_missing_required_args(span));
                        break;
                    }
                }
            }
            self.record_callee_effects(&sig, args, span);
            // <eff E = Pure> 泛型函数：从第 1 个参数推断 E 的 effect 集，
            // 然后将 E 替换到所有参数类型中做类型检查。
            let eff_var = self.eff_var_of_fqn_or_sig(&sig);
            let mut subst_params = sig.params.clone();
            if let Some(ev) = eff_var {
                // 计算预实参类型（块实参按函数类型参数降级为 lambda）。
                let arg_types_pre: Vec<TypeId> = args
                    .iter()
                    .enumerate()
                    .map(|(i, a)| self.walk_call_arg_for_inference(a, sig.params.get(i).copied()))
                    .collect();
                let inferred = self.infer_eff_var_from_arg(&ev, &sig, &arg_types_pre);
                if !inferred.is_empty() {
                    subst_params = sig
                        .params
                        .iter()
                        .map(|&pt| self.subst_eff_var_in_type(pt, &ev, &inferred))
                        .collect();
                }
            }
            return self.check_call_args_with(
                subst_params,
                sig.return_ty,
                args,
                span,
                sig.has_vararg,
            );
        }
        // 多候选：尝试每个候选的 expected param count 来降级 lambda 实参。
        // 先用默认方式走一遍实参类型（无 expected 感知），收集 arg_types。
        let arg_types_default: Vec<TypeId> =
            args.iter().map(|a| self.walk_expr(&a.value)).collect();
        // 尝试用每个候选的 expected param count 重新降级 lambda 实参。
        let mut best_applicable: Vec<(usize, Vec<TypeId>)> = Vec::new();
        for (sig_idx, s) in non_generic.iter().enumerate() {
            let n = s.params.len();
            let mut min_arity = s.has_defaults.iter().position(|d| *d).unwrap_or(n);
            if s.has_vararg && n > 0 {
                min_arity = min_arity.min(n - 1);
            }
            let arity_ok = if s.has_vararg {
                arg_types_default.len() >= min_arity && arg_types_default.len() <= n
            } else {
                arg_types_default.len() == n
            };
            if !arity_ok {
                continue;
            }
            // 重新降级 lambda 实参（带 expected param count）。
            let arg_types: Vec<TypeId> = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let expected_pt = s
                        .params
                        .get(i)
                        .and_then(|&pt| self.extract_expected_param_type(pt));
                    if let ast::ExprKind::Lambda(lam) = &a.value.kind
                        && lam.params.is_empty()
                    {
                        return self.type_lambda_with_expected(lam, expected_pt);
                    }
                    arg_types_default[i]
                })
                .collect();
            let all_match = arg_types
                .iter()
                .zip(&s.params)
                .all(|(a, p)| self.assignable(*a, *p));
            if all_match {
                best_applicable.push((sig_idx, arg_types));
            }
        }
        let applicable: Vec<usize> = best_applicable.iter().map(|(i, _)| *i).collect();
        match applicable.len() {
            0 => {
                // 构建候选详情 + related labels。
                let fqn_text = self.env.interner.resolve(fqn).to_string();
                let fun_name = fqn_text.rsplit('.').next().unwrap_or(&fqn_text);
                let decls = self.env.index.lookup_funs(fqn);
                let mut msg = "没有匹配的重载候选".to_string();
                let mut diag = scoop2_base::diag::Diagnostic::error(
                    "scoop::typecheck::no_applicable_overload",
                    "",
                )
                .with_primary(span, "这里");
                for (i, sig) in non_generic.iter().enumerate() {
                    let params = sig
                        .params
                        .iter()
                        .map(|p| self.describe(*p))
                        .collect::<Vec<_>>()
                        .join(", ");
                    msg.push_str(&format!("\n  {fun_name}({params})"));
                    if sig.params.len() == arg_types_default.len() {
                        for (j, (at, pt)) in arg_types_default.iter().zip(&sig.params).enumerate() {
                            if !self.assignable(*at, *pt) {
                                let pname = sig
                                    .param_names
                                    .get(j)
                                    .map(|s| self.env.interner.resolve(*s))
                                    .unwrap_or("");
                                msg.push_str(&format!(
                                    "\n    argument {} type {} is not a subtype of parameter `{pname}` type {}",
                                    j + 1,
                                    self.describe(*at),
                                    self.describe(*pt),
                                ));
                            }
                        }
                    }
                    if let Some(d) = decls.get(i) {
                        diag = diag.with_related(d.span, format!("{fun_name}({params})"));
                    }
                }
                diag.message = msg;
                self.diags.push(diag);
                non_generic
                    .first()
                    .map(|s| s.return_ty)
                    .unwrap_or_else(|| self.env.store.nothing())
            }
            1 => non_generic[applicable[0]].return_ty,
            _ => {
                // 特殊性：若存在唯一最具体的候选，直接选择。
                let sig_refs: Vec<&Signature> =
                    applicable.iter().map(|&i| &non_generic[i]).collect();
                if let Some(idx) = self.select_most_specific(&sig_refs) {
                    self.record_callee_effects(sig_refs[idx], args, span);
                    return sig_refs[idx].return_ty;
                }
                let fqn_text = self.env.interner.resolve(fqn).to_string();
                let fun_name = fqn_text.rsplit('.').next().unwrap_or(&fqn_text);
                let decls = self.env.index.lookup_funs(fqn);
                let first = sig_refs[0];
                let second = sig_refs[1];
                let first_params: Vec<String> =
                    first.params.iter().map(|p| self.describe(*p)).collect();
                let second_params: Vec<String> =
                    second.params.iter().map(|p| self.describe(*p)).collect();
                let first_str = format!("{fun_name}({})", first_params.join(", "));
                let second_str = format!("{fun_name}({})", second_params.join(", "));
                let diff_pos = first_params
                    .iter()
                    .zip(&second_params)
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                let msg = format!(
                    "重载决议歧义：{first_str} vs {second_str}: position {diff_pos}\n  reason: 多个候选同等匹配"
                );
                let mut diag = scoop2_base::diag::Diagnostic::error(
                    "scoop::typecheck::ambiguous_overload",
                    msg,
                )
                .with_primary(span, "这里");
                for &sig_idx in applicable.iter() {
                    let sig = &non_generic[sig_idx];
                    let params: Vec<String> =
                        sig.params.iter().map(|p| self.describe(*p)).collect();
                    let label = format!("{fun_name}({})", params.join(", "));
                    if let Some(d) = decls.get(sig_idx) {
                        diag = diag.with_related(d.span, label);
                    }
                }
                self.diags.push(diag);
                non_generic[applicable[0]].return_ty
            }
        }
    }

    /// 若 `ft` 是函数类型，返回其（克隆的）`FunctionType`。
    fn function_type_of(&self, ft: TypeId) -> Option<FunctionType> {
        match self.env.store.kind(ft) {
            TypeKind::Ref(crate::ty::RefTypeKind::Function(f)) => Some(f.clone()),
            _ => None,
        }
    }

    /// 按位置检查调用实参（M2：仅位置实参；命名 / spread 在 M3）。
    fn check_call_args(
        &mut self,
        param_types: Vec<TypeId>,
        return_ty: TypeId,
        args: &[CallArg],
        span: Span,
    ) -> TypeId {
        self.check_call_args_with(param_types, return_ty, args, span, false)
    }

    /// `has_vararg`：签名最后一个参数是 vararg（可接收任意多余位置实参）。
    fn check_call_args_with(
        &mut self,
        param_types: Vec<TypeId>,
        return_ty: TypeId,
        args: &[CallArg],
        span: Span,
        has_vararg: bool,
    ) -> TypeId {
        // 命名实参形态校验（结构性，不受 lenient 影响）：重复 / 位置在命名之后。
        let mut seen_named = false;
        let mut seen_names: HashSet<Symbol> = HashSet::new();
        for a in args {
            if let Some(name) = &a.name {
                if !seen_names.insert(name.symbol) {
                    self.diags.push(diagnostics::call_arg_duplicate(
                        self.env.interner.resolve(name.symbol),
                        name.span,
                    ));
                }
                seen_named = true;
            } else if seen_named {
                self.diags
                    .push(diagnostics::call_arg_positional_after_named(a.span));
            }
        }
        if param_types.len() != args.len() && !args.iter().any(|a| a.name.is_some()) {
            // 实参过多（args.len() > param_types.len()）对非 vararg 函数始终是真实错误
            //（重载不会有更多形参），即使 lenient 也报；vararg 函数合法接收多余实参。
            // 实参过少可能是重载候选未定 / 默认值，lenient 时抑制。
            let too_many = args.len() > param_types.len();
            let too_few = args.len() < param_types.len();
            // 实参过少 + 非 vararg → 始终真实错误（即使 lenient）。
            // 实参过多 + 非 vararg → 始终真实错误。
            // vararg 函数：多余实参合法；不足实参也合法（vararg 可为空）。
            let always_report = (too_few && !has_vararg) || (too_many && !has_vararg);
            if (!self.lenient_type_errors || always_report) && !has_vararg {
                self.diags.push(diagnostics::call_arity_mismatch(
                    param_types.len(),
                    args.len(),
                    span,
                ));
            }
            for a in args {
                self.walk_expr(&a.value);
            }
            return return_ty;
        }
        for (i, a) in args.iter().enumerate() {
            // 若实参是无参 lambda 且期望参数是函数类型，注入 expected param count。
            let expected_pt = param_types
                .get(i)
                .and_then(|&pt| self.extract_expected_param_type(pt));
            let at = if let ast::ExprKind::Lambda(lam) = &a.value.kind {
                if lam.params.is_empty() {
                    self.type_lambda_with_expected(lam, expected_pt)
                } else {
                    self.walk_expr(&a.value)
                }
            } else {
                // 裸块实参 `{ ... }` 且期望参数是函数类型 → 按零参 lambda 降级（捕获 effect）。
                self.walk_call_arg_for_inference(a, param_types.get(i).copied())
            };
            if a.name.is_some() {
                continue;
            }
            // spread 实参 `*arr` / `*(1, 2, 3)`：若函数有 vararg，spread 实参的元素类型需匹配 vararg 参数类型。
            if a.is_spread
                && has_vararg
                && let Some(&pt) = param_types.get(i)
            {
                // 提取 Array<T> / MutableArray<T> 的元素类型，或 Tuple 的各元素类型。
                let elem_tys: Vec<TypeId> = match self.env.store.kind(at) {
                    TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) if n.args.len() == 1 => {
                        vec![n.args[0]]
                    }
                    TypeKind::Value(crate::ty::ValueTypeKind::Tuple(els)) => els.clone(),
                    _ => vec![],
                };
                if !elem_tys.is_empty() {
                    let all_ok = elem_tys.iter().all(|&et| self.assignable(et, pt));
                    if !all_ok && !self.lenient_type_errors {
                        self.diags.push(diagnostics::call_arg_type_mismatch(
                            &self.describe(pt),
                            &self.describe(at),
                            a.value.span,
                        ));
                    }
                    continue;
                }
            }
            if let Some(&pt) = param_types.get(i) {
                // 数组字面量实参 → MutableArray 参数：上下文相关转换。
                if matches!(&a.value.kind, ExprKind::ArrayLit(_))
                    && self.is_array_lit_to_mutable_array(Some(&a.value), pt, at)
                {
                    continue;
                }
                // Continuation answer-type 精确比较（escape continuation binder 的 answer type
                // 必须与参数的 answer type 一致；assignable 对同 FQN nominal 不比较类型实参）。
                // 同时比较 effect row：found 的 effect row 必须是 expected 的子集（子效应关系）。
                if let (
                    TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nf)),
                    TypeKind::Ref(crate::ty::RefTypeKind::Nominal(ne)),
                ) = (self.env.store.kind(at), self.env.store.kind(pt))
                {
                    let is_cont = |fqn: scoop2_base::Symbol| {
                        self.env.interner.resolve(fqn).ends_with(".Continuation")
                    };
                    if nf.fqn == ne.fqn && is_cont(nf.fqn) {
                        let answer_mismatch = nf.args.len() >= 2
                            && ne.args.len() >= 2
                            && nf.args[1] != ne.args[1]
                            && !self.assignable(nf.args[1], ne.args[1]);
                        // effect row 子集检查：found.eff 必须是 expected.eff 的子集。
                        let eff_mismatch = match (&nf.eff, &ne.eff) {
                            (Some(fe), Some(ee)) => !fe.is_subset_of(ee),
                            (Some(fe), None) => !fe.is_pure(),
                            _ => false,
                        };
                        if answer_mismatch || eff_mismatch {
                            self.diags.push(diagnostics::call_arg_type_mismatch(
                                &self.describe(pt),
                                &self.describe(at),
                                a.value.span,
                            ));
                            continue;
                        }
                    }
                }
                if !self.assignable(at, pt)
                    && (!self.lenient_type_errors
                        || init_expr_is_unambiguous_literal(&a.value.kind))
                {
                    self.diags.push(diagnostics::call_arg_type_mismatch(
                        &self.describe(pt),
                        &self.describe(at),
                        a.value.span,
                    ));
                }
            }
        }
        return_ty
    }

    // ---- 成员访问（M2 part 2：属性 / 字段读取） ----

    /// 解析成员查找的所有者 FQN：若是 typealias，展开到底层类型 FQN（递归）。
    fn resolve_member_owner_fqn(&self, receiver_ty: TypeId) -> Option<scoop2_base::Symbol> {
        let mut fqn = nominal_fqn_of(self.env.store.kind(receiver_ty))?;
        // typealias 展开（最多 8 层防环）。
        for _ in 0..8 {
            if let Some((rhs, _)) = self.env.type_alias(fqn) {
                // 取 RHS 的路径 FQN（假设底层是 Path 类型）。
                if let crate::syntax::ast::TypeRefKind::Path { path, .. } = &rhs.kind
                    && let Some(last) = path.segments.last()
                {
                    // 尝试候选 FQN：裸名 / package prefix / scoop.core。
                    let name = self.env.interner.resolve(last.symbol);
                    let candidates = [
                        name.to_string(),
                        format!("{}.{}", self.package_prefix, name),
                        format!("scoop.core.{}", name),
                        format!("scoop.collections.{}", name),
                    ];
                    let mut found = None;
                    for cand in &candidates {
                        if let Some(f) = self.env.interner.get(cand) {
                            found = Some(f);
                            break;
                        }
                    }
                    if let Some(new_fqn) = found {
                        if new_fqn == fqn {
                            break;
                        }
                        fqn = new_fqn;
                        continue;
                    }
                }
            }
            break;
        }
        Some(fqn)
    }

    fn member_access_type(
        &mut self,
        receiver_ty: TypeId,
        member: MemberName,
        _span: Span,
    ) -> TypeId {
        let MemberName::Named(name) = member else {
            return self.env.store.nothing();
        };
        let fqn = self.resolve_member_owner_fqn(receiver_ty);
        let Some(fqn) = fqn else {
            return self.env.store.nothing();
        };
        match self.env.member_type(fqn, name.symbol) {
            Some(t) => t,
            None => {
                // 名为方法的成员在值位访问（未调用）→ 暂不支持。
                if self.env.member_signatures(fqn, name.symbol).is_some() {
                    let mname = self.env.interner.resolve(name.symbol);
                    self.diags
                        .push(diagnostics::unsupported_member_access(mname, name.span));
                }
                self.env.store.nothing()
            }
        }
    }

    /// struct 字面量 `Type { field: value, ... }`：返回该 nominal 类型，
    /// 检查每个字段值可赋值到对应成员类型。
    fn type_struct_lit(
        &mut self,
        name: ast::Ident,
        fields: &[StructLitField],
        span: Span,
    ) -> TypeId {
        let Some(fqn) = self.callee_type_fqn(name.symbol) else {
            return self.unsupported("该 struct 字面量的类型名", span);
        };
        // struct 字面量只能用于 struct 类型。
        let is_struct = self
            .env
            .index
            .category(fqn)
            .is_some_and(|c| matches!(c, crate::resolve::symbol::NominalCategory::Struct));
        if !is_struct {
            if !self.lenient_type_errors {
                self.diags.push(diagnostics::struct_lit_not_struct(
                    self.env.interner.resolve(name.symbol),
                    span,
                ));
            }
            // 非 struct 类型：walk values 后返回 Nothing（避免假 initializer_type_mismatch）。
            for f in fields {
                self.walk_expr(&f.value);
            }
            return self.env.store.nothing();
        }
        let type_name = self.env.interner.resolve(name.symbol).to_string();
        let mut seen: HashSet<Symbol> = HashSet::new();
        for f in fields {
            let field_name_text = self.env.interner.resolve(f.name.symbol).to_string();
            // 重复字段检查。
            if seen.contains(&f.name.symbol) {
                self.diags.push(diagnostics::struct_lit_duplicate_field(
                    &field_name_text,
                    f.span,
                ));
                self.walk_expr(&f.value);
                continue;
            }
            seen.insert(f.name.symbol);
            match self.env.member_type(fqn, f.name.symbol) {
                Some(expected) => {
                    let vt = self.walk_expr(&f.value);
                    if !self.assignable(vt, expected) {
                        self.diags.push(diagnostics::struct_lit_field_type_mismatch(
                            &field_name_text,
                            &self.describe(expected),
                            &self.describe(vt),
                            f.value.span,
                        ));
                    }
                }
                None => {
                    self.walk_expr(&f.value);
                    self.diags.push(diagnostics::struct_lit_unknown_field(
                        &field_name_text,
                        &type_name,
                        f.span,
                    ));
                }
            }
        }
        let nominal = NominalType {
            fqn,
            args: vec![],
            eff: None,
        };
        // 缺少必填字段检查（lenient 模式下跳过——成员注册不完整）。
        // 反射 meta 类型（FieldMeta/VariantMeta/ParamMeta 等）在 splice field access 等
        // 上下文中可仅提供部分字段（仅 name 有意义），跳过缺失字段检查。
        let fqn_name = self.env.interner.resolve(fqn);
        let is_meta_type = fqn_name.ends_with(".FieldMeta")
            || fqn_name.ends_with(".VariantMeta")
            || fqn_name.ends_with(".ParamMeta")
            || fqn_name.ends_with(".AnnotationMeta")
            || fqn_name.ends_with(".TypeMeta")
            || fqn_name.ends_with(".FunctionMeta")
            || fqn_name.ends_with(".PropertyMeta");
        if !self.lenient_type_errors
            && is_struct
            && !is_meta_type
            && let Some(members) = self.env.member_types(fqn)
        {
            let missing: Vec<String> = members
                .keys()
                .filter(|k| !seen.contains(k))
                .map(|k| self.env.interner.resolve(*k).to_string())
                .collect();
            if !missing.is_empty() {
                self.diags.push(diagnostics::struct_lit_missing_fields(
                    &missing.join(", "),
                    Span::new(span.end.saturating_sub(1), span.end),
                ));
            }
        }
        if self.env.is_reference_nominal(fqn) {
            self.env.store.ref_nominal(nominal)
        } else {
            self.env.store.value_nominal(nominal)
        }
    }

    /// `with` 更新表达式 `base with { path: v, ... }`：返回 base 类型。
    /// 校验路径冲突、字段存在性、值类型匹配（短路：首个错误即返回）。
    /// enum base 的逐段校验暂未实现（variant payload 未在 Index 登记）。
    fn type_with_update(&mut self, base: &Expr, updates: &[ast::WithUpdateField]) -> TypeId {
        let base_ty = self.walk_expr(base);
        let base_kind = self.with_update_aggregate_kind(base_ty);
        if base_kind.is_none() {
            self.diags.push(diagnostics::with_update_base_not_supported(
                &self.describe(base_ty),
                base.span,
            ));
            for u in updates {
                self.walk_expr(&u.value);
            }
            return base_ty;
        }
        let base_kind = base_kind.unwrap();

        // Phase 1：路径冲突筛查（精确重复 / 前缀包含）。
        let mut seen_exact: HashMap<String, Span> = HashMap::new();
        let mut seen_paths: Vec<(Vec<String>, Span)> = Vec::new();
        for u in updates {
            let segs: Vec<String> = u
                .path
                .segments
                .iter()
                .map(|s| self.member_name_text(s))
                .collect();
            let path = segs.join(".");
            if seen_exact.contains_key(&path) {
                self.diags
                    .push(diagnostics::with_update_duplicate_path(&path, u.path.span));
                for u2 in updates {
                    self.walk_expr(&u2.value);
                }
                return base_ty;
            }
            let mut overlapped: Option<(String, String)> = None;
            for (prev, _pspan) in &seen_paths {
                if is_strict_prefix(prev, &segs) {
                    overlapped = Some((prev.join("."), path.clone()));
                    break;
                }
                if is_strict_prefix(&segs, prev) {
                    overlapped = Some((path.clone(), prev.join(".")));
                    break;
                }
            }
            if let Some((parent, child)) = overlapped {
                self.diags.push(diagnostics::with_update_overlapping_paths(
                    &parent,
                    &child,
                    u.path.span,
                ));
                for u2 in updates {
                    self.walk_expr(&u2.value);
                }
                return base_ty;
            }
            seen_exact.insert(path, u.path.span);
            seen_paths.push((segs, u.path.span));
        }

        // Phase 2：逐项路径解析 + 值类型检查。
        for (idx, u) in updates.iter().enumerate() {
            if !self.check_with_update_path(&base_kind, u) {
                // 短路：首个错误后停止后续更新（仍需消费剩余 value 的副作用）。
                for u2 in &updates[idx + 1..] {
                    self.walk_expr(&u2.value);
                }
                return base_ty;
            }
        }
        base_ty
    }

    /// `with` 更新项的路径解析；返回是否成功（false = 已报错，调用方短路）。
    fn check_with_update_path(
        &mut self,
        base_kind: &WithUpdateAggregate,
        u: &ast::WithUpdateField,
    ) -> bool {
        // enum base：首段必须是已知 variant；选中 variant 后必须给出 payload 字段路径。
        if let WithUpdateAggregate::Enum(enum_fqn) = base_kind {
            if let Some(first) = u.path.segments.first() {
                let first_name = self.member_name_text(first);
                let is_variant = self.env.enum_variants(*enum_fqn).is_some_and(|vs| {
                    vs.iter().any(|&v| {
                        let vn = self.env.interner.resolve(v);
                        vn == first_name || vn.ends_with(&format!(".{first_name}"))
                    })
                });
                if !is_variant {
                    self.diags.push(diagnostics::with_update_unknown_variant(
                        self.member_name_span(first),
                    ));
                } else if u.path.segments.len() < 2 {
                    self.diags
                        .push(diagnostics::with_update_variant_field_required(
                            u.value.span,
                        ));
                }
            }
            self.walk_expr(&u.value);
            return true;
        }
        if u.path.segments.is_empty() {
            self.walk_expr(&u.value);
            return true;
        }
        let mut current_kind = base_kind.clone();
        let last = u.path.segments.len() - 1;
        for (idx, seg) in u.path.segments.iter().enumerate() {
            // enum 中段同样跳过（variant payload 未登记）。
            if matches!(current_kind, WithUpdateAggregate::Enum(_)) {
                self.walk_expr(&u.value);
                return true;
            }
            let is_last = idx == last;
            let owner_name = current_kind.owner_name(self.env);
            match self.with_update_step(&current_kind, seg, &owner_name) {
                Ok(field_ty) => {
                    if is_last {
                        // 值类型检查（精确相等 + 字面量吸收）。
                        let found = self.walk_expr(&u.value);
                        if found != field_ty && !self.literal_absorbs_to(&u.value, field_ty) {
                            let field_name = self.member_name_text(seg);
                            self.diags
                                .push(diagnostics::with_update_field_type_mismatch(
                                    &owner_name,
                                    &field_name,
                                    &self.describe(field_ty),
                                    &self.describe(found),
                                    u.value.span,
                                ));
                            return false;
                        }
                    } else {
                        // 中间段：必须是 aggregate 才能继续。
                        match self.with_update_aggregate_kind(field_ty) {
                            Some(k) => current_kind = k,
                            None => {
                                // 非 aggregate 中间段：停止（无专用 fixture，安静停止）。
                                self.walk_expr(&u.value);
                                return true;
                            }
                        }
                    }
                }
                Err(diag) => {
                    self.diags.push(diag);
                    self.walk_expr(&u.value);
                    return false;
                }
            }
        }
        true
    }

    /// 单段路径解析：返回该段的字段类型，或一个诊断（仅 Struct/Tuple；Enum 由调用方跳过）。
    #[allow(clippy::result_large_err)]
    fn with_update_step(
        &self,
        kind: &WithUpdateAggregate,
        seg: &ast::MemberName,
        owner_name: &str,
    ) -> Result<TypeId, scoop2_base::diag::Diagnostic> {
        match kind {
            WithUpdateAggregate::Struct(fqn) => {
                if let ast::MemberName::Named(name) = seg {
                    match self.env.member_type(*fqn, name.symbol) {
                        Some(t) => Ok(t),
                        None => Err(diagnostics::with_update_unknown_field(
                            owner_name,
                            &self.member_name_text(seg),
                            self.member_name_span(seg),
                        )),
                    }
                } else {
                    Err(diagnostics::with_update_unknown_field(
                        owner_name,
                        &self.member_name_text(seg),
                        self.member_name_span(seg),
                    ))
                }
            }
            WithUpdateAggregate::Tuple(els) => {
                let text = self.member_name_text(seg);
                if let Some(new) = old_tuple_member_replacement(&text) {
                    return Err(diagnostics::tuple_member_old_syntax(
                        &text,
                        &new,
                        self.member_name_span(seg),
                    ));
                }
                let Some(idx) = text.parse::<usize>().ok() else {
                    return Err(diagnostics::with_update_unknown_field(
                        owner_name,
                        &text,
                        self.member_name_span(seg),
                    ));
                };
                els.get(idx).copied().ok_or_else(|| {
                    diagnostics::with_update_unknown_field(
                        owner_name,
                        &text,
                        self.member_name_span(seg),
                    )
                })
            }
            WithUpdateAggregate::Enum(_) => {
                // invariant: 调用方对 Enum base/中段均提前跳过，不会进入此分支。
                unreachable!("enum with-update step is skipped by caller")
            }
        }
    }

    /// base / 中间类型是否为 `with` 支持的 aggregate 值类型。
    fn with_update_aggregate_kind(&self, ty: TypeId) -> Option<WithUpdateAggregate> {
        match self.env.store.kind(ty) {
            TypeKind::Value(crate::ty::ValueTypeKind::Tuple(els)) => {
                Some(WithUpdateAggregate::Tuple(els.to_vec()))
            }
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
                match self.env.index.category(n.fqn) {
                    Some(crate::resolve::symbol::NominalCategory::Struct) => {
                        Some(WithUpdateAggregate::Struct(n.fqn))
                    }
                    Some(crate::resolve::symbol::NominalCategory::Enum) => {
                        Some(WithUpdateAggregate::Enum(n.fqn))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn member_name_text(&self, m: &ast::MemberName) -> String {
        match m {
            ast::MemberName::Named(id) => self.env.interner.resolve(id.symbol).to_string(),
            ast::MemberName::TupleIndex { value, .. } => value.to_string(),
        }
    }

    fn member_name_span(&self, m: &ast::MemberName) -> Span {
        match m {
            ast::MemberName::Named(id) => id.span,
            ast::MemberName::TupleIndex { span, .. } => *span,
        }
    }

    /// 字面量吸收：无后缀 int → 任意整数类型；无后缀 float → Float32。
    fn literal_absorbs_to(&self, expr: &Expr, expected: TypeId) -> bool {
        match &expr.kind {
            ExprKind::IntLit(l) => {
                l.suffix.is_none()
                    && matches!(
                        self.env.store.kind(expected),
                        TypeKind::Value(
                            crate::ty::ValueTypeKind::Int
                                | crate::ty::ValueTypeKind::UInt
                                | crate::ty::ValueTypeKind::IntN(_)
                                | crate::ty::ValueTypeKind::UIntN(_)
                        )
                    )
            }
            ExprKind::FloatLit(_) => {
                matches!(
                    self.env.store.kind(expected),
                    TypeKind::Value(crate::ty::ValueTypeKind::Float32)
                )
            }
            _ => false,
        }
    }

    /// lambda → 函数类型（参数有标注则降级；无标注 → Nothing，精确推断推迟）。
    /// `expected_param_type`: 期望函数类型的第 1 参数类型（来自调用实参位置的签名）。
    /// 无参 lambda 在期望为 1 参数时注入隐式 `it`，类型为 expected_param_type。
    fn type_lambda_with_expected(
        &mut self,
        lambda: &ast::LambdaExpr,
        expected_param_type: Option<TypeId>,
    ) -> TypeId {
        let inject_it = lambda.params.is_empty() && expected_param_type.is_some();
        let mut param_types: Vec<TypeId> = Vec::new();
        if inject_it && let Some(it_sym) = self.env.interner.get("it") {
            let it_ty = expected_param_type.unwrap_or_else(|| self.env.store.nothing());
            self.locals.insert(it_sym, it_ty);
            param_types.push(it_ty);
        }
        for p in &lambda.params {
            let ty = match &p.ty {
                Some(t) => self.lower_type(t),
                None => self.env.store.nothing(),
            };
            self.locals.insert(p.name.symbol, ty);
            param_types.push(ty);
        }
        // lambda 体内 `return` 不是 non-local return（spec §7.2 / T4013）。
        let saved_fn = self.in_function_body;
        self.in_function_body = false;
        self.lambda_depth += 1;
        // 记录 body 前的 performed_effects 长度；lambda body 执行的 effect 用于构造函数类型。
        // 不增加 effect_suspend_depth（需要采集 body 的 effect 以构造函数类型的 effect row）。
        let body_eff_start = self.performed_effects.len();
        let return_ty = match &lambda.body {
            ast::LambdaBody::Block(b) => self.walk_block(b),
            ast::LambdaBody::Expr(e) => self.walk_expr(e),
        };
        self.lambda_depth -= 1;
        self.in_function_body = saved_fn;
        // 清理隐式 `it`。
        if inject_it && let Some(it_sym) = self.env.interner.get("it") {
            self.locals.remove(&it_sym);
        }
        // 收集 lambda body 执行的 effect，构造函数类型的 effect row。
        // 这些 effect 不应传播到外层函数（lambda 的 effect 由其函数类型携带，
        // 由调用点的 assignability 检查决定是否允许）。
        let body_effects = self.performed_effects[body_eff_start..].to_vec();
        self.performed_effects.truncate(body_eff_start);
        // 仅保留可解析为 nominal 的 effect（effect 类型名 → FQN nominal TypeId）。
        let mut eff_terms: Vec<TypeId> = Vec::new();
        for (eff_name, _) in &body_effects {
            if let Some(ty) = self.effect_type_to_typeid(eff_name)
                && !eff_terms.contains(&ty)
            {
                eff_terms.push(ty);
            }
        }
        let effects = crate::ty::EffectRow::from_terms(eff_terms);
        self.env.store.function(FunctionType {
            receiver: None,
            params: param_types,
            return_ty,
            effects,
            closed: false,
        })
    }

    /// lambda → 函数类型（无 expected type 感知，默认不注入 `it`）。
    fn type_lambda(&mut self, lambda: &ast::LambdaExpr) -> TypeId {
        self.type_lambda_with_expected(lambda, None)
    }

    /// 从期望的函数类型参数中提取第 1 参数类型（用于隐式 `it` 注入）。
    fn extract_expected_param_type(&self, expected_fn_ty: TypeId) -> Option<TypeId> {
        if let TypeKind::Ref(crate::ty::RefTypeKind::Function(ft)) =
            self.env.store.kind(expected_fn_ty)
        {
            ft.params.first().copied()
        } else {
            None
        }
    }

    /// 计算调用实参的类型（用于 eff 变量推断）。
    /// 若实参是裸块（`{ ... }`）且期望参数是函数类型，则按零参 lambda 降级，
    /// 以捕获块体执行的 effect（供 `<eff E = Pure>` 推断）。
    fn walk_call_arg_for_inference(
        &mut self,
        arg: &CallArg,
        expected_pt: Option<TypeId>,
    ) -> TypeId {
        let is_block = matches!(&arg.value.kind, ExprKind::Block(_) | ExprKind::DoBlock(_));
        let expected_is_fn = expected_pt.is_some_and(|pt| {
            matches!(
                self.env.store.kind(pt),
                TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
            )
        });
        if is_block
            && expected_is_fn
            && let ExprKind::Block(b) | ExprKind::DoBlock(b) = &arg.value.kind
        {
            // 合成零参 lambda：block 体作为 lambda body。
            let lam = ast::LambdaExpr {
                is_safe: false,
                params: vec![],
                body: ast::LambdaBody::Block(b.clone()),
            };
            return self.type_lambda_with_expected(&lam, None);
        }
        self.walk_expr(&arg.value)
    }

    /// 类型是否含 effect 行参数（函数类型的 effect row 中的 `TypeKind::Param`）。
    /// 用于 `<eff E = Pure>` 函数的方法调用 lenient 判定。
    fn type_contains_eff_param(&self, ty: TypeId) -> bool {
        match self.env.store.kind(ty) {
            TypeKind::Param(_) => true,
            TypeKind::Ref(crate::ty::RefTypeKind::Function(ft)) => {
                ft.effects
                    .terms
                    .iter()
                    .any(|&t| matches!(self.env.store.kind(t), TypeKind::Param(_)))
                    || ft.params.iter().any(|&p| self.type_contains_eff_param(p))
                    || self.type_contains_eff_param(ft.return_ty)
            }
            _ => false,
        }
    }

    /// 星投影读视图兼容性：元素类型为引用类型或 `Option<Ref>` 可作为 `*` 的读视图。
    fn is_star_read_compatible_elem(&self, elem: TypeId) -> bool {
        match self.env.store.kind(elem) {
            TypeKind::Ref(_) => true,
            TypeKind::Value(crate::ty::ValueTypeKind::Option(inner)) => {
                matches!(self.env.store.kind(*inner), TypeKind::Ref(_))
            }
            _ => false,
        }
    }

    /// `when` 表达式：walk subject + 每分支体；返回各分支体的 LUB（同类型 → 该类型；否则 Unit）。
    fn type_when(&mut self, subject: &Expr, arms: &[ast::WhenArm], span: Span) -> TypeId {
        let subject_ty = self.walk_expr(subject);
        let mut arm_types: Vec<TypeId> = Vec::new();
        for arm in arms {
            self.bind_pattern_locals_typed(&arm.pat, subject_ty);
            self.check_when_pattern(&arm.pat, subject_ty);
            if let Some(g) = &arm.guard {
                let gt = self.walk_expr(g);
                // guard 必须是 Bool；Nothing（未解析的 pattern binder）也算非 Bool。
                if !matches!(
                    self.env.store.kind(gt),
                    TypeKind::Value(crate::ty::ValueTypeKind::Bool)
                ) {
                    self.diags
                        .push(diagnostics::when_guard_not_bool(&self.describe(gt), g.span));
                }
            }
            arm_types.push(self.walk_expr(&arm.body));
        }
        // 穷尽性检查：Bool 需 true+false；Option 需 Some+None；否则需要 else/通配。
        if let Some(missing) = self.when_missing_case(subject_ty, arms) {
            if missing == "else" {
                self.diags.push(diagnostics::when_missing_else(span));
            } else {
                self.diags
                    .push(diagnostics::when_non_exhaustive_missing_variants_detail(
                        &missing, span,
                    ));
            }
        }
        if arm_types.is_empty() {
            return self.env.store.unit();
        }
        let first = arm_types[0];
        if arm_types.iter().all(|&t| t == first) {
            first
        } else {
            self.env.store.unit()
        }
    }

    /// 把模式中的绑定名登记为 Nothing 类型局部（精确模式类型在 M5 补齐）。
    fn bind_pattern_locals(&mut self, pat: &ast::Pattern) {
        let mut names: Vec<Symbol> = Vec::new();
        collect_pattern_binders(&pat.kind, &mut names);
        for name in names {
            self.locals.insert(name, self.env.store.nothing());
        }
    }

    /// 把 when 分支模式中的绑定名按 variant payload 字段类型登记。
    /// `Ready(f)` → f 的类型为 `() -> Int`（variant payload 第 1 字段类型）。
    fn bind_pattern_locals_typed(&mut self, pat: &ast::Pattern, subject_ty: TypeId) {
        use crate::syntax::ast::PatternKind;
        match &pat.kind {
            PatternKind::Variant { path, args } => {
                // 从 subject enum 的 variant payload 提取字段类型。
                if let Some(enum_fqn) = nominal_fqn_of(self.env.store.kind(subject_ty))
                    && let Some(last_seg) = path.segments.last()
                {
                    let variant_name = self.env.interner.resolve(last_seg.symbol);
                    // 查找 variant 的 payload 字段类型。
                    // 构造 variant FQN: `<enum_fqn>.<variant_name>`。
                    let variant_fqn_candidates = [format!(
                        "{}.{}",
                        self.env.interner.resolve(enum_fqn),
                        variant_name
                    )];
                    for vfqn_text in &variant_fqn_candidates {
                        if let Some(vfqn) = self.env.interner.get(vfqn_text)
                            && let Some(fields) = self.env.member_types(vfqn)
                        {
                            // 按位置绑定 binder 到字段类型。
                            let field_tys: Vec<TypeId> = fields.values().copied().collect();
                            if let Some(args) = args {
                                for (i, arg) in args.iter().enumerate() {
                                    if let PatternKind::Bind(ident) = &arg.kind
                                        && let Some(&ft) = field_tys.get(i)
                                    {
                                        self.locals.insert(ident.symbol, ft);
                                    }
                                }
                            }
                            return;
                        }
                    }
                }
                // 回退：Nothing。
                self.bind_pattern_locals(pat);
            }
            PatternKind::Or(alts) => {
                for alt in alts {
                    self.bind_pattern_locals_typed(alt, subject_ty);
                }
            }
            _ => {
                self.bind_pattern_locals(pat);
            }
        }
    }

    /// 校验 when 分支模式与 subject 类型匹配（or-pattern 递归检查每个分支）。
    fn check_when_pattern(&mut self, pat: &ast::Pattern, subject_ty: TypeId) {
        use crate::syntax::ast::{PatternKind, PatternLiteral};
        if self.env.store.is_nothing(subject_ty) {
            return;
        }
        let kind = self.env.store.kind(subject_ty);
        match &pat.kind {
            PatternKind::Or(alts) => {
                // or-pattern 不允许引入 binder（当前语言 contract 下 scope/dominance 未定义）。
                for alt in alts {
                    let mut binders: Vec<Symbol> = Vec::new();
                    collect_pattern_binders(&alt.kind, &mut binders);
                    if !binders.is_empty() {
                        self.diags
                            .push(diagnostics::when_or_pattern_binder_not_allowed(alt.span));
                    }
                }
                // variant arity 检查：若所有分支是同一 enum 的 variant，校验 args 数量与
                // payload 一致（裸名形式对带 payload 的 variant 是 arity 不匹配）。
                // 先把 enum_fqn（Copy 的 Symbol）取出，释放对 store 的不可变借用，再做可变调用。
                let enum_fqn = nominal_fqn_of(kind);
                if let Some(fqn) = enum_fqn {
                    for alt in alts {
                        if let PatternKind::Variant { path, args } = &alt.kind
                            && let Some(seg) = path.segments.last()
                        {
                            let provided = args.as_ref().map(|a| a.len()).unwrap_or(0);
                            let has_rest = args.as_ref().is_some_and(|a| {
                                a.iter().any(|e| matches!(e.kind, PatternKind::Rest))
                            });
                            let expected = self.variant_payload_arity(fqn, seg.symbol);
                            if let Some(exp) = expected
                                && provided != exp
                                && !has_rest
                            {
                                self.diags
                                    .push(diagnostics::when_variant_pat_arity_mismatch(alt.span));
                            }
                        }
                    }
                }
                // 递归校验每个分支（放在最后，避免与上面的不可变借用冲突）。
                for alt in alts {
                    self.check_when_pattern(alt, subject_ty);
                }
            }
            PatternKind::Literal(PatternLiteral::String(_)) => {
                let nominal_name = match kind {
                    TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => {
                        Some(self.env.interner.resolve(n.fqn))
                    }
                    _ => None,
                };
                let is_string = matches!(kind, TypeKind::Ref(crate::ty::RefTypeKind::String))
                    || nominal_name.is_some_and(|n| n == "String" || n.ends_with(".String"));
                if !is_string {
                    self.diags
                        .push(diagnostics::when_string_pat_not_string(pat.span));
                }
            }
            PatternKind::Variant { path, args } => {
                // enum nominal 或内建 Option<T>（variant 模式可用的 subject）。
                let is_enum = matches!(kind, TypeKind::Value(crate::ty::ValueTypeKind::Option(_)))
                    || nominal_fqn_of(kind)
                        .and_then(|fqn| self.env.index.category(fqn))
                        .is_some_and(|c| {
                            matches!(c, crate::resolve::symbol::NominalCategory::Enum)
                        });
                if !is_enum {
                    self.diags
                        .push(diagnostics::when_variant_pat_not_enum(pat.span));
                } else if let Some(args) = args {
                    // 带 rest 时，对已知 arity 的 variant（Option 等）检查前缀不超过 payload 数。
                    let has_rest = args.iter().any(|a| matches!(a.kind, PatternKind::Rest));
                    if has_rest
                        && let Some(last) = path.segments.last()
                        && let Some(arity) = self.known_enum_variant_arity(last.symbol)
                    {
                        let non_rest = args
                            .iter()
                            .filter(|a| !matches!(a.kind, PatternKind::Rest))
                            .count();
                        if non_rest > arity {
                            self.diags
                                .push(diagnostics::when_variant_pat_too_short(pat.span));
                        }
                    }
                }
            }
            PatternKind::Tuple(elems) => {
                let is_tuple = matches!(kind, TypeKind::Value(crate::ty::ValueTypeKind::Tuple(_)));
                if !is_tuple {
                    self.diags
                        .push(diagnostics::when_tuple_pat_not_tuple(pat.span));
                } else if let TypeKind::Value(crate::ty::ValueTypeKind::Tuple(subject_elems)) = kind
                {
                    // 带 rest 时，pattern 前缀（非 rest 元素）不能超过 tuple 长度。
                    let has_rest = elems.iter().any(|e| matches!(e.kind, PatternKind::Rest));
                    if has_rest {
                        let non_rest = elems
                            .iter()
                            .filter(|e| !matches!(e.kind, PatternKind::Rest))
                            .count();
                        if non_rest > subject_elems.len() {
                            self.diags.push(diagnostics::when_tuple_pat_too_short(
                                non_rest,
                                subject_elems.len(),
                                pat.span,
                            ));
                        }
                    }
                }
            }
            PatternKind::Is(ty_ref) => {
                // `when is T`：runtime type test。spec 限定只对**引用**类型可用，
                // 且函数类型即使闭合也不支持（effect row 只存在于编译期）。
                // value 类型（struct/enum/标量）无运行期 tag，前端拒绝。
                let ty = self.lower_type(ty_ref);
                match self.env.store.kind(ty) {
                    TypeKind::Value(_) => {
                        self.diags
                            .push(diagnostics::when_type_pattern_runtime_test_not_supported(
                                pat.span,
                            ));
                    }
                    TypeKind::Ref(crate::ty::RefTypeKind::Function(ft)) => {
                        if ft.closed && ft.effects.is_pure() {
                            self.diags
                                .push(diagnostics::when_function_type_pattern_not_supported(
                                    pat.span,
                                ));
                        } else {
                            self.diags.push(
                                diagnostics::when_effectful_function_type_pattern_not_supported(
                                    pat.span,
                                ),
                            );
                        }
                    }
                    // 引用 nominal 类型（class/interface/object）：合法的 runtime test。
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// 校验 `val` 解构的 variant/tuple pattern 与 initializer 类型匹配。
    fn check_val_pattern(&mut self, pat: &ast::Pattern, init_ty: TypeId) {
        use crate::syntax::ast::PatternKind;
        if self.env.store.is_nothing(init_ty) {
            return;
        }
        // tuple pattern：initializer 必须是 tuple。
        if let PatternKind::Tuple(_) = &pat.kind {
            let kind = self.env.store.kind(init_ty);
            if !matches!(kind, TypeKind::Value(crate::ty::ValueTypeKind::Tuple(_))) {
                self.diags
                    .push(diagnostics::val_tuple_pat_not_tuple(pat.span));
            }
            return;
        }
        let path = match &pat.kind {
            PatternKind::Variant { path, .. } => path,
            _ => return,
        };
        let seg_name =
            |s: &crate::syntax::ast::Ident| self.env.interner.resolve(s.symbol).to_string();
        let kind = self.env.store.kind(init_ty);
        // Option initializer：Some/None 已知，跳过 prefix/variant 校验。
        if matches!(kind, TypeKind::Value(crate::ty::ValueTypeKind::Option(_))) {
            return;
        }
        // initializer 必须是 enum nominal。
        let Some(enum_fqn) = nominal_fqn_of(kind).filter(|&fqn| {
            self.env
                .index
                .category(fqn)
                .is_some_and(|c| matches!(c, crate::resolve::symbol::NominalCategory::Enum))
        }) else {
            self.diags
                .push(diagnostics::val_variant_pat_not_enum(pat.span));
            return;
        };
        let enum_name = self.env.interner.resolve(enum_fqn);
        let enum_short = enum_name.rsplit('.').next().unwrap_or(enum_name);
        // 多段前缀：除最后一段外必须匹配 enum 名。
        if path.segments.len() >= 2 {
            let prefix = path.segments[..path.segments.len() - 1]
                .iter()
                .map(seg_name)
                .collect::<Vec<_>>()
                .join(".");
            if prefix != enum_short && prefix != enum_name {
                self.diags
                    .push(diagnostics::val_variant_pat_enum_mismatch(pat.span));
                return;
            }
        }
        // 最后一段必须是该 enum 的 variant。
        let variant_name = path.segments.last().map(seg_name).unwrap_or_default();
        let known = self.env.enum_variants(enum_fqn).is_some_and(|vs| {
            vs.iter().any(|&v| {
                let vn = self.env.interner.resolve(v);
                vn == variant_name || vn.ends_with(&format!(".{variant_name}"))
            })
        });
        if !known {
            let span = path.segments.last().map(|s| s.span).unwrap_or(pat.span);
            self.diags
                .push(diagnostics::val_variant_pat_unknown_variant(span));
        }
    }

    /// 检查类型参数上的方法调用：where 约束链中是否有该方法。
    /// 无约束时报告不可调用。
    fn check_typeparam_method(
        &mut self,
        tp: &crate::ty::TypeParamType,
        method_name: Symbol,
        span: Span,
    ) -> TypeId {
        let method_text = self.env.interner.resolve(method_name).to_string();
        // 搜索所有 type_constraints，找到约束名 == tp.name 的 Type bound。
        let bound_refs: Vec<crate::syntax::ast::TypeRef> = self.env.find_type_param_bounds(tp.name);
        for bound_ref in &bound_refs {
            let bound_ty = self.lower_type(bound_ref);
            let kind = self.env.store.kind(bound_ty);
            let fqn = nominal_fqn_of(kind).or_else(|| scalar_fqn(kind, self.env.interner));
            if let Some(fqn) = fqn {
                let exists = self.method_exists_in_chain(fqn, method_name);
                if exists {
                    let ret = self
                        .env
                        .member_signatures(fqn, method_name)
                        .and_then(|s| s.first())
                        .map(|s| s.return_ty);
                    if let Some(ret) = ret {
                        return ret;
                    }
                    return self.env.store.nothing();
                }
            }
        }
        if !self.lenient_type_errors {
            self.diags
                .push(diagnostics::callee_not_callable(&method_text, span));
        }
        self.env.store.nothing()
    }

    /// 在 FQN 及其超类型链中查找方法。
    fn method_exists_in_chain(
        &self,
        fqn: scoop2_base::Symbol,
        method: scoop2_base::Symbol,
    ) -> bool {
        if self.env.member_signatures(fqn, method).is_some() {
            return true;
        }
        for &sup in self.env.index.supertypes_of(fqn) {
            if self.method_exists_in_chain(sup, method) {
                return true;
            }
        }
        false
    }

    /// 方法调用（`receiver.method(args)` / `receiver[args]` / `receiver until arg`）：
    /// 查 receiver nominal 类型的成员函数签名，按适用性选择，返回返回类型。
    /// `has_explicit_type_args`：调用是否带显式类型实参（`obj.m<T>(x)`），用于抑制
    /// generic_type_arg_not_inferred（显式提供时不需推断）。
    #[allow(clippy::too_many_arguments)]
    fn method_call_return_type(
        &mut self,
        receiver_ty: TypeId,
        method_name: Symbol,
        arg_types: &[TypeId],
        args: &[CallArg],
        span: Span,
        name_span: Span,
        has_explicit_type_args: bool,
    ) -> TypeId {
        // Nothing 接收者（类型未知）→ lenient（返回 Nothing，不报错误）。
        if self.env.store.is_nothing(receiver_ty) {
            return self.env.store.nothing();
        }
        // TypeParam 接收者：检查 where 约束是否有该方法。
        let tp_clone = if let TypeKind::Param(tp) = self.env.store.kind(receiver_ty) {
            Some(crate::ty::TypeParamType {
                name: tp.name,
                file: tp.file,
                span: tp.span,
            })
        } else {
            None
        };
        if let Some(tp) = tp_clone {
            return self.check_typeparam_method(&tp, method_name, span);
        }
        let fqn = self
            .resolve_member_owner_fqn(receiver_ty)
            .or_else(|| nominal_fqn_of(self.env.store.kind(receiver_ty)))
            .or_else(|| scalar_fqn(self.env.store.kind(receiver_ty), self.env.interner));
        let Some(fqn) = fqn else {
            return self.unsupported("该接收者的方法调用", span);
        };
        let sigs_with_owners = self.env.member_signatures_with_owners(fqn, method_name);
        let sigs: Vec<Signature> = sigs_with_owners.iter().map(|(_, s)| s.clone()).collect();
        // 扩展方法：从 Index 的 extensions 表查找同名扩展函数。
        // 扩展函数的签名已由 register_top_level_signatures 注册到 member_signatures
        //（collect 阶段 resolve_extensions 将 receiver.FQN.name 注册为成员）。
        // 但如果扩展在 sysroot（受信任文件），其 body 可能未注册签名。
        // 此时 member_signatures_with_inherited 已包含扩展（resolve_extensions 注册）。
        if sigs.is_empty() {
            // 可能是 callable field（`holder.f()` 中 f 是函数类型字段）：检查成员类型。
            if let Some(field_ty) = self
                .resolve_member_owner_fqn(receiver_ty)
                .and_then(|fqn| self.env.member_type(fqn, method_name))
                && let Some(ft) = self.function_type_of(field_ty)
            {
                // callable field：按函数类型检查实参与返回。
                return self.check_call_args_with(ft.params, ft.return_ty, args, span, false);
            }
            // 接收者类型确定（非 Nothing / 非 Param）却找不到方法 → 真实错误。
            // lenient 模式抑制（可能是重载决议未定等假阳性）。
            if !self.lenient_type_errors {
                let method_text = self.env.interner.resolve(method_name);
                let callee_fqn = if self.package_prefix.is_empty() {
                    method_text.to_string()
                } else {
                    format!("{}.{}", self.package_prefix, method_text)
                };
                self.diags
                    .push(diagnostics::callee_not_callable(&callee_fqn, name_span));
            }
            return self.env.store.nothing();
        }
        let non_generic: Vec<Signature> = sigs
            .iter()
            .filter(|s| s.type_param_count == 0)
            .cloned()
            .collect();
        if non_generic.is_empty() {
            // 泛型方法：尝试用 receiver 类型实参替换类型参数后返回。
            // 扩展方法 `MutableArray<T>.toArray(): Array<T>` 调用时 receiver 类型已知，
            // 可推断 T 的值并替换返回类型。
            let generic_sigs: Vec<Signature> = sigs
                .into_iter()
                .filter(|s| s.type_param_count > 0)
                .collect();
            if generic_sigs.len() == 1 {
                let sig = &generic_sigs[0];
                // 检查返回类型是否含 Param；若有，尝试从 receiver_ty 提取类型实参替换。
                if let TypeKind::Ref(crate::ty::RefTypeKind::Function(_)) =
                    self.env.store.kind(receiver_ty)
                {
                    // 函数类型 receiver 不是常规 nominal；lenient。
                    return self.env.store.nothing();
                }
                // 尝试从 receiver_ty 提取类型实参并替换 sig.return_ty 中的 Param。
                let receiver_args =
                    nominal_fqn_of(self.env.store.kind(receiver_ty)).and_then(|_| {
                        match self.env.store.kind(receiver_ty) {
                            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => {
                                Some(n.args.clone())
                            }
                            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
                                Some(n.args.clone())
                            }
                            _ => None,
                        }
                    });
                if let Some(rargs) = receiver_args
                    && !rargs.is_empty()
                {
                    // sig.return_ty 中的 Param 替换为 rargs 对应位置。
                    // 简化：对所有 Param 类型，用 receiver 的第一个类型实参替换。
                    let subst_arg = rargs
                        .first()
                        .copied()
                        .unwrap_or_else(|| self.env.store.nothing());
                    let substituted = self.subst_all_params(sig.return_ty, subst_arg);
                    return substituted;
                }
                // 无 receiver 类型实参可推断：若返回类型仍含 Param 且无法从参数推断，报错。
                // 但显式类型实参已提供（`obj.m<T>(x)`）时不报（T 已显式给出）。
                let return_has_param =
                    matches!(self.env.store.kind(sig.return_ty), TypeKind::Param(_));
                let params_have_matching_param = sig
                    .params
                    .iter()
                    .any(|p| matches!(self.env.store.kind(*p), TypeKind::Param(_)));
                if return_has_param
                    && !params_have_matching_param
                    && !has_explicit_type_args
                    && arg_types.iter().all(|a| !self.env.store.is_nothing(*a))
                {
                    self.diags
                        .push(diagnostics::generic_type_arg_not_inferred(span));
                }
            }
            return self.env.store.nothing();
        }
        if non_generic.len() == 1 {
            let sig = &non_generic[0];
            // Unit sugar：0 实参 + 1 个 Unit 参数 → 匹配。
            // 也处理泛型方法签名中的 TypeParam 参数（无法知道实例化后是否为 Unit → lenient）。
            if arg_types.is_empty()
                && sig.params.len() == 1
                && (sig.params[0] == self.env.store.unit()
                    || matches!(self.env.store.kind(sig.params[0]), TypeKind::Param(_)))
            {
                return sig.return_ty;
            }
            // 泛型方法签名：param 类型含 TypeParam → lenient on arity + types。
            // 注意：仅当方法自身非泛型（type_param_count == 0）且 receiver 有类型实参时，
            // 尝试用 receiver 类型实参替换 Param（如 AtomicValue<Pair>.cas(Box<T>, T)）。
            let has_param = sig.params.iter().any(|p| {
                // 顶层 Param 或函数类型 effect row 含 Param（`<eff E>` 的 E 在 effect row 中）。
                self.type_contains_eff_param(*p)
            });
            if has_param {
                // 仅对非泛型方法（type_param_count == 0）的 Param 用 receiver 类型实参替换。
                // 泛型方法（如 <U: ref> forwardRefMember(x: U)）的 U 是方法自己的类型参数，
                // 不能用 receiver 类型实参替换。
                if sig.type_param_count == 0
                    && self.env.interner.resolve(method_name) != "resume"
                    && let Some(rargs) =
                        nominal_fqn_of(self.env.store.kind(receiver_ty)).and_then(|_| {
                            match self.env.store.kind(receiver_ty) {
                                TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => {
                                    Some(n.args.clone())
                                }
                                TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
                                    Some(n.args.clone())
                                }
                                _ => None,
                            }
                        })
                    && !rargs.is_empty()
                {
                    let subst_arg = rargs
                        .first()
                        .copied()
                        .unwrap_or_else(|| self.env.store.nothing());
                    let subst_params: Vec<TypeId> = sig
                        .params
                        .iter()
                        .map(|&p| self.subst_all_params(p, subst_arg))
                        .collect();
                    let matches = subst_params.len() == arg_types.len()
                        && arg_types
                            .iter()
                            .zip(&subst_params)
                            .all(|(a, p)| self.assignable(*a, *p));
                    if !matches {
                        let method_text = self.env.interner.resolve(method_name);
                        let recv_desc = self
                            .resolve_member_owner_fqn(receiver_ty)
                            .map(|fqn| self.env.interner.resolve(fqn).to_string())
                            .unwrap_or_else(|| self.describe(receiver_ty));
                        let param_descs: Vec<String> =
                            subst_params.iter().map(|&p| self.describe(p)).collect();
                        let candidate_desc = format!(
                            "{method_text}({recv_desc}{})",
                            if param_descs.is_empty() {
                                String::new()
                            } else {
                                format!(", {}", param_descs.join(", "))
                            }
                        );
                        let reason = if subst_params.len() != arg_types.len() {
                            format!(
                                "实参数量 {} 与形参数量 {} 不匹配",
                                arg_types.len(),
                                subst_params.len()
                            )
                        } else {
                            let mut found = None;
                            for (i, (&at, &pt)) in arg_types.iter().zip(&subst_params).enumerate() {
                                if !self.assignable(at, pt) {
                                    let pname = sig
                                        .param_names
                                        .get(i)
                                        .map(|s| self.env.interner.resolve(*s).to_string())
                                        .unwrap_or_else(|| format!("param{i}"));
                                    found = Some(format!(
                                        "argument {} type {} is not a subtype of parameter `{}` type {}",
                                        i + 2,
                                        self.describe(at),
                                        pname,
                                        self.describe(pt)
                                    ));
                                    break;
                                }
                            }
                            found.unwrap_or_else(|| "参数类型不匹配".to_string())
                        };
                        self.diags.push(diagnostics::no_applicable_overload_member(
                            &candidate_desc,
                            &reason,
                            span,
                            sig.decl_span,
                        ));
                        return sig.return_ty;
                    }
                    self.record_callee_effects(sig, args, span);
                    return sig.return_ty;
                }
                // eff-param 方法（`<eff E>`）：从 lambda 实参推断 E 的 effect 并传播到外层。
                self.record_callee_effects(sig, args, span);
                return sig.return_ty;
            }
            // 若唯一候选的 arity / 参数类型不匹配，走 no_applicable_overload
            //（成员调用是重载决议场景，不匹配应报 no_applicable_overload 而非 call_arg_type_mismatch）。
            let matches = sig.params.len() == arg_types.len()
                && arg_types
                    .iter()
                    .zip(&sig.params)
                    .all(|(a, p)| self.assignable(*a, *p));
            if !matches {
                // 构造候选描述 `method(receiver_fqn, ParamType1, ParamType2, ...)`。
                let method_text = self.env.interner.resolve(method_name);
                let recv_desc = self
                    .resolve_member_owner_fqn(receiver_ty)
                    .map(|fqn| self.env.interner.resolve(fqn).to_string())
                    .unwrap_or_else(|| self.describe(receiver_ty));
                let param_descs: Vec<String> =
                    sig.params.iter().map(|&p| self.describe(p)).collect();
                let candidate_desc = format!(
                    "{method_text}({recv_desc}{})",
                    if param_descs.is_empty() {
                        String::new()
                    } else {
                        format!(", {}", param_descs.join(", "))
                    }
                );
                // 找到首个不匹配的参数作为原因。
                let reason = if sig.params.len() != arg_types.len() {
                    format!(
                        "实参数量 {} 与形参数量 {} 不匹配",
                        arg_types.len(),
                        sig.params.len()
                    )
                } else {
                    let mut found = None;
                    for (i, (&at, &pt)) in arg_types.iter().zip(&sig.params).enumerate() {
                        if !self.assignable(at, pt) {
                            let pname = sig
                                .param_names
                                .get(i)
                                .map(|s| self.env.interner.resolve(*s).to_string())
                                .unwrap_or_else(|| format!("param{i}"));
                            // 成员调用的 receiver 计为 arg 1，首个显式实参计为 arg 2。
                            found = Some(format!(
                                "argument {} type {} is not a subtype of parameter `{}` type {}",
                                i + 2,
                                self.describe(at),
                                pname,
                                self.describe(pt)
                            ));
                            break;
                        }
                    }
                    found.unwrap_or_else(|| "参数类型不匹配".to_string())
                };
                self.diags.push(diagnostics::no_applicable_overload_member(
                    &candidate_desc,
                    &reason,
                    span,
                    sig.decl_span,
                ));
                return sig.return_ty;
            }
            self.record_callee_effects(sig, args, span);
            return sig.return_ty;
        }
        let applicable: Vec<(&Signature, Symbol)> = sigs_with_owners
            .iter()
            .filter(|(_, s)| s.type_param_count == 0)
            .filter(|(_, s)| {
                s.params.len() == arg_types.len()
                    && arg_types
                        .iter()
                        .zip(&s.params)
                        .all(|(a, p)| self.assignable(*a, *p))
            })
            .map(|(owner, s)| (s, *owner))
            .collect();
        match applicable.len() {
            0 => {
                self.diags.push(diagnostics::no_applicable_overload(span));
                non_generic[0].return_ty
            }
            _ => {
                // 特异性选择（含 receiver FQN）+ effect 传播。
                let sig_refs: Vec<&Signature> = applicable.iter().map(|(s, _)| *s).collect();
                if let Some(idx) = self.select_most_specific_with_receiver(&applicable) {
                    self.record_callee_effects(sig_refs[idx], args, span);
                    sig_refs[idx].return_ty
                } else {
                    // receiver-aware 选择无唯一胜者 → 报歧义（receiver 差异使其不可比较）。
                    let method_text = self.env.interner.resolve(method_name);
                    // 构造候选对比描述：`A.m(Pa) vs B.m(Pb): position 0`。
                    let mut vs_parts: Vec<String> = Vec::new();
                    for (sig, owner) in &applicable {
                        let owner_text = self.env.interner.resolve(*owner);
                        let params: Vec<String> =
                            sig.params.iter().map(|&p| self.describe(p)).collect();
                        vs_parts.push(format!("{owner_text}.{method_text}({})", params.join(", ")));
                    }
                    let vs_desc = vs_parts.join(" vs ");
                    let mut diag = scoop2_base::diag::Diagnostic::error(
                        "scoop::typecheck::ambiguous_overload",
                        format!("重载决议歧义：{vs_desc}: position 0"),
                    )
                    .with_primary(span, "这里")
                    .with_help(
                        "reason: receiver 更具体但参数更宽泛（或反之），无法确定唯一最具体候选",
                    );
                    // 添加候选声明处的 related 标注。
                    for (sig, owner) in &applicable {
                        let owner_text = self.env.interner.resolve(*owner);
                        let params: Vec<String> =
                            sig.params.iter().map(|&p| self.describe(p)).collect();
                        let label = format!(
                            "{method_text}({}{})",
                            owner_text,
                            if params.is_empty() {
                                String::new()
                            } else {
                                format!(", {}", params.join(", "))
                            }
                        );
                        diag = diag.with_related(sig.decl_span, label);
                    }
                    self.diags.push(diag);
                    sig_refs[0].return_ty
                }
            }
        }
    }

    /// 从适用候选中选择最具体的（spec P3 §4.5）。
    /// A 比 B 更具体 ⟺ ∀i: A.params[i] <: B.params[i]。
    /// 选择唯一最具体的候选（每个参数均更具体）；若无唯一胜者返回 None（歧义）。
    fn select_most_specific(&self, candidates: &[&Signature]) -> Option<usize> {
        let n = candidates.len();
        if n <= 1 {
            return Some(0);
        }
        // a 比 b 更具体：a 的每个参数是 b 对应参数的（严格，无字面量吸收）子类型。
        let more_specific = |a: &Signature, b: &Signature| -> bool {
            a.params.len() == b.params.len()
                && a.params
                    .iter()
                    .zip(&b.params)
                    .all(|(ap, bp)| self.assignable_strict(*ap, *bp))
        };
        let mut winner = vec![true; n];
        for i in 0..n {
            for j in 0..n {
                if i != j
                    && more_specific(candidates[j], candidates[i])
                    && !more_specific(candidates[i], candidates[j])
                {
                    // j 严格比 i 更具体 → i 不可能胜出。
                    winner[i] = false;
                }
            }
        }
        let winners: Vec<usize> = winner
            .iter()
            .enumerate()
            .filter(|(_, w)| **w)
            .map(|(i, _)| i)
            .collect();
        if winners.len() == 1 {
            Some(winners[0])
        } else {
            None
        }
    }

    /// 同 [`select_most_specific`]，但把 receiver 的声明者 FQN 作为位置 0 参与特异性比较
    ///（扩展方法 `String.mix(Any)` vs `Any.mix(String)` 的 receiver 差异决定歧义）。
    fn select_most_specific_with_receiver(
        &self,
        candidates: &[(&Signature, scoop2_base::Symbol)],
    ) -> Option<usize> {
        let n = candidates.len();
        if n <= 1 {
            return Some(0);
        }
        // a 比 b 更具体：receiver FQN a 是 b 的子类型，且每个参数 a[i] <: b[i]。
        let more_specific = |a: &(&Signature, scoop2_base::Symbol),
                             b: &(&Signature, scoop2_base::Symbol)| {
            let (sa, oa) = a;
            let (sb, ob) = b;
            // receiver 子类型：oa（声明者）是 ob 的子类型。
            let recv_more = self.fqn_is_subtype(*oa, *ob);
            let params_more = sa.params.len() == sb.params.len()
                && sa
                    .params
                    .iter()
                    .zip(&sb.params)
                    .all(|(ap, bp)| self.assignable_strict(*ap, *bp));
            recv_more && params_more
        };
        let mut winner = vec![true; n];
        for i in 0..n {
            for j in 0..n {
                if i != j
                    && more_specific(&candidates[j], &candidates[i])
                    && !more_specific(&candidates[i], &candidates[j])
                {
                    winner[i] = false;
                }
            }
        }
        let winners: Vec<usize> = winner
            .iter()
            .enumerate()
            .filter(|(_, w)| **w)
            .map(|(i, _)| i)
            .collect();
        if winners.len() == 1 {
            Some(winners[0])
        } else {
            None
        }
    }
}

/// 收集模式中的绑定名（`Bind` / struct 简写）。
fn collect_pattern_binders(kind: &ast::PatternKind, out: &mut Vec<Symbol>) {
    match kind {
        ast::PatternKind::Bind(name) => out.push(name.symbol),
        ast::PatternKind::Tuple(elems) => {
            for e in elems {
                collect_pattern_binders(&e.kind, out);
            }
        }
        ast::PatternKind::Struct { fields, .. } => {
            for f in fields {
                match &f.pattern {
                    None => out.push(f.name.symbol),
                    Some(p) => collect_pattern_binders(&p.kind, out),
                }
            }
        }
        ast::PatternKind::Variant {
            args: Some(elems), ..
        } => {
            for e in elems {
                collect_pattern_binders(&e.kind, out);
            }
        }
        ast::PatternKind::Or(alts) => {
            for a in alts {
                collect_pattern_binders(&a.kind, out);
            }
        }
        _ => {}
    }
}

/// 内建标量的 nominal FQN（`Int` → `scoop.core.Int` 等），用于子类型检查。
pub(super) fn scalar_fqn(kind: &TypeKind, interner: &scoop2_base::Interner) -> Option<Symbol> {
    use crate::ty::{RefTypeKind, ValueTypeKind};
    let name: &'static str = match kind {
        TypeKind::Value(ValueTypeKind::Int) => "scoop.core.Int",
        TypeKind::Value(ValueTypeKind::UInt) => "scoop.core.UInt",
        TypeKind::Value(ValueTypeKind::Bool) => "scoop.core.Bool",
        TypeKind::Value(ValueTypeKind::Char) => "scoop.core.Char",
        TypeKind::Value(ValueTypeKind::Float64) => "scoop.core.Float64",
        TypeKind::Value(ValueTypeKind::Float32) => "scoop.core.Float32",
        TypeKind::Value(ValueTypeKind::IntN(8)) => "scoop.core.Int8",
        TypeKind::Value(ValueTypeKind::IntN(16)) => "scoop.core.Int16",
        TypeKind::Value(ValueTypeKind::IntN(32)) => "scoop.core.Int32",
        TypeKind::Value(ValueTypeKind::IntN(64)) => "scoop.core.Int64",
        TypeKind::Value(ValueTypeKind::UIntN(8)) => "scoop.core.UInt8",
        TypeKind::Value(ValueTypeKind::UIntN(16)) => "scoop.core.UInt16",
        TypeKind::Value(ValueTypeKind::UIntN(32)) => "scoop.core.UInt32",
        TypeKind::Value(ValueTypeKind::UIntN(64)) => "scoop.core.UInt64",
        TypeKind::Ref(RefTypeKind::String) => "scoop.core.String",
        TypeKind::Ref(RefTypeKind::Any) => "scoop.core.Any",
        _ => return None,
    };
    interner.get(name)
}

/// 若 `kind` 是 nominal（ref 或 value），返回其 FQN。
pub(super) fn nominal_fqn_of(kind: &TypeKind) -> Option<Symbol> {
    match kind {
        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
        | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(n.fqn),
        _ => None,
    }
}

/// nominal 类型的类型实参切片（ref/value 均覆盖）。
pub(super) fn nominal_args_of(kind: &TypeKind) -> Option<&[TypeId]> {
    match kind {
        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
        | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(&n.args),
        _ => None,
    }
}

/// 从声明的 effect 行提取 effect 短名集合（`Pure` 排除；FQN 去前缀）。
pub(super) fn extract_effect_row_names(
    eff: Option<&ast::EffectRowExpr>,
    interner: &scoop2_base::Interner,
) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    if let Some(eff) = eff {
        for term in &eff.terms {
            if let Some(seg) = term.path.segments.last() {
                let n = interner.resolve(seg.symbol);
                let s = n.strip_prefix("scoop.core.").unwrap_or(n);
                if s != "Pure" {
                    names.insert(s.to_string());
                }
            }
        }
    }
    names
}

/// 声明的 effect 行是否为 Pure（无行，或所有项均为 `Pure`，且非闭合行）。
pub(super) fn effect_row_expr_is_pure(
    eff: Option<&ast::EffectRowExpr>,
    interner: &scoop2_base::Interner,
) -> bool {
    match eff {
        None => true,
        Some(eff) => {
            eff.closed.is_none()
                && eff.terms.iter().all(|t| {
                    t.path
                        .segments
                        .last()
                        .map(|s| interner.resolve(s.symbol) == "Pure")
                        .unwrap_or(true)
                })
        }
    }
}

/// `with` 更新的 aggregate 基类型。
#[derive(Clone)]
enum WithUpdateAggregate {
    Struct(Symbol),
    Tuple(Vec<TypeId>),
    Enum(Symbol),
}

impl WithUpdateAggregate {
    fn owner_name(&self, env: &TypeEnv) -> String {
        match self {
            WithUpdateAggregate::Struct(fqn) | WithUpdateAggregate::Enum(fqn) => {
                env.interner.resolve(*fqn).to_string()
            }
            WithUpdateAggregate::Tuple(_) => "tuple".to_string(),
        }
    }
}

/// `a` 是否为 `b` 的严格前缀（逐段字符串相等，且更短）。
fn is_strict_prefix(a: &[String], b: &[String]) -> bool {
    if a.len() >= b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| x == y)
}

/// 旧式 tuple 成员写法 `_N` → 新写法 `N`（仅 `_` + 数字）。
fn old_tuple_member_replacement(text: &str) -> Option<String> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(digits.to_string())
}

/// where 约束的人类可读描述（诊断用）。
fn generic_bound_desc(
    bound: &crate::syntax::ast::GenericBound,
    interner: &scoop2_base::Interner,
) -> String {
    use crate::syntax::ast::{GenericBound, TypeRefKind};
    match bound {
        GenericBound::Ref(_) => "ref".to_string(),
        GenericBound::Value(_) => "value".to_string(),
        GenericBound::Type(t) => match &t.kind {
            TypeRefKind::Path { path, .. } => path
                .segments
                .iter()
                .map(|s| interner.resolve(s.symbol))
                .collect::<Vec<_>>()
                .join("."),
            _ => format!("{:?}", t.kind),
        },
    }
}

/// 是否为带非 Pure effect row 的函数类型（显式 `as`/`as?` 不支持）。
fn is_effectful_function_type(env: &TypeEnv, id: TypeId) -> bool {
    matches!(
        env.store.kind(id),
        TypeKind::Ref(crate::ty::RefTypeKind::Function(f)) if !f.effects.is_pure()
    )
}

/// TypeRef 是否为带非 Pure effect 行的函数类型（直接看 AST effect 注解，无需降级 effect）。
fn is_effectful_function_type_ref(ty: &TypeRef, interner: &scoop2_base::Interner) -> bool {
    let effect = match &ty.kind {
        TypeRefKind::Function { effect, .. } | TypeRefKind::ReceiverFunction { effect, .. } => {
            effect.as_ref()
        }
        _ => return false,
    };
    let Some(eff) = effect else {
        return false;
    };
    // 任一非 Pure 项 → effectful。
    eff.terms.iter().any(|t| {
        !t.path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == "Pure")
    })
}

/// 标量类别（用于运算结果判定）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScalarCat {
    Int,
    Float,
    Bool,
    Char,
    String,
    Other,
}

fn scalar_cat(kind: &TypeKind) -> ScalarCat {
    match kind {
        TypeKind::Value(crate::ty::ValueTypeKind::Int)
        | TypeKind::Value(crate::ty::ValueTypeKind::UInt)
        | TypeKind::Value(crate::ty::ValueTypeKind::IntN(_))
        | TypeKind::Value(crate::ty::ValueTypeKind::UIntN(_)) => ScalarCat::Int,
        TypeKind::Value(crate::ty::ValueTypeKind::Float64)
        | TypeKind::Value(crate::ty::ValueTypeKind::Float32) => ScalarCat::Float,
        TypeKind::Value(crate::ty::ValueTypeKind::Bool) => ScalarCat::Bool,
        TypeKind::Value(crate::ty::ValueTypeKind::Char) => ScalarCat::Char,
        TypeKind::Ref(crate::ty::RefTypeKind::String) => ScalarCat::String,
        _ => ScalarCat::Other,
    }
}

/// 标量二元运算结果类型（M1）。同类型标量；不满足 → `None`（调用方报 unsupported）。
fn builtin_binop(
    op: BinaryOp,
    cat: ScalarCat,
    lt: TypeId,
    rt: TypeId,
    bool_ty: TypeId,
) -> Option<TypeId> {
    use BinaryOp as B;
    if lt != rt {
        return None;
    }
    let numeric = matches!(cat, ScalarCat::Int | ScalarCat::Float);
    match op {
        B::Mul
        | B::Div
        | B::Rem
        | B::Add
        | B::Sub
        | B::Shl
        | B::Shr
        | B::BitAnd
        | B::BitXor
        | B::BitOr => {
            if numeric {
                Some(lt)
            } else {
                None
            }
        }
        B::Lt | B::Le | B::Gt | B::Ge => {
            // 排序比较不支持 Bool（Bool 仅参与相等比较）。
            if matches!(
                cat,
                ScalarCat::Int | ScalarCat::Float | ScalarCat::Char | ScalarCat::String
            ) {
                Some(bool_ty)
            } else {
                None
            }
        }
        B::Eq | B::Ne => {
            if matches!(
                cat,
                ScalarCat::Int
                    | ScalarCat::Float
                    | ScalarCat::Bool
                    | ScalarCat::Char
                    | ScalarCat::String
            ) {
                Some(bool_ty)
            } else {
                None
            }
        }
        B::LogAnd | B::LogOr => {
            if cat == ScalarCat::Bool {
                Some(bool_ty)
            } else {
                None
            }
        }
        B::Range | B::Elvis => None,
    }
}

/// 标量一元运算结果类型（M1）。
fn builtin_unary(op: UnaryOp, cat: ScalarCat, t: TypeId, bool_ty: TypeId) -> Option<TypeId> {
    match op {
        UnaryOp::Not => {
            if cat == ScalarCat::Bool {
                Some(bool_ty)
            } else {
                None
            }
        }
        UnaryOp::Neg => {
            if matches!(cat, ScalarCat::Int | ScalarCat::Float) {
                Some(t)
            } else {
                None
            }
        }
        UnaryOp::BitNot => {
            if cat == ScalarCat::Int {
                Some(t)
            } else {
                None
            }
        }
    }
}

/// BinaryOp → 运算符重载方法名。
fn binop_to_method_name(op: BinaryOp) -> Option<&'static str> {
    use BinaryOp as B;
    Some(match op {
        B::Add => "plus",
        B::Sub => "minus",
        B::Mul => "times",
        B::Div => "div",
        B::Rem => "rem",
        B::Shl => "shl",
        B::Shr => "shr",
        B::BitAnd => "and",
        B::BitXor => "xor",
        B::BitOr => "or",
        B::Lt => "lt",
        B::Le => "le",
        B::Gt => "gt",
        B::Ge => "ge",
        B::Eq => "equals",
        B::Ne => "notEquals",
        B::Range => "rangeTo",
        B::LogAnd | B::LogOr | B::Elvis => return None,
    })
}

/// UnaryOp → 运算符重载方法名（`-` → `unaryMinus`，`~` → `inv`，`!` → None (逻辑)）。
fn unop_to_method_name(op: UnaryOp) -> Option<&'static str> {
    Some(match op {
        UnaryOp::Neg => "unaryMinus",
        UnaryOp::BitNot => "inv",
        UnaryOp::Not => return None,
    })
}

/// BinaryOp → 源码符号（诊断用）。
fn binop_symbol(op: BinaryOp) -> &'static str {
    use BinaryOp as B;
    match op {
        B::Add => "+",
        B::Sub => "-",
        B::Mul => "*",
        B::Div => "/",
        B::Rem => "%",
        B::Shl => "<<",
        B::Shr => ">>",
        B::BitAnd => "&",
        B::BitXor => "^",
        B::BitOr => "|",
        B::Lt => "<",
        B::Le => "<=",
        B::Gt => ">",
        B::Ge => ">=",
        B::Eq => "==",
        B::Ne => "!=",
        B::Range => "..",
        B::LogAnd => "&&",
        B::LogOr => "||",
        B::Elvis => "?:",
    }
}

/// UnaryOp → 源码符号（诊断用）。
fn unop_symbol(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::BitNot => "~",
        UnaryOp::Not => "!",
    }
}

/// 抑制未用导入警告（NodeId 在 M1 尚未写回 typed 结果）。
#[allow(dead_code)]
fn _node_id_used(_id: NodeId) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::Index;
    use crate::resolve::collect::collect_file;
    use crate::resolve::imports::ImportTable;
    use crate::resolve::output::Resolution;
    use crate::resolve::symbol::ConeKind;
    use crate::ty::TypeKind;
    use scoop2_base::{FileId, SourceFile};
    use scoop2_syntax::parser::parse_file;

    /// 解析 + resolve + typecheck 一个 `fun main()` 程序；返回 (诊断码, store)。
    fn check(src: &str) -> (Vec<String>, scoop2_base::Interner) {
        let result = parse_file(&SourceFile::new_virtual("<mem>", src));
        let mut interner = result.interner;
        let mut index = Index::new();
        let mut diags = scoop2_base::diag::DiagnosticSink::new();
        let cone = index.intern_cone("test", ConeKind::Bin);
        let prefix = crate::resolve::collect::package_prefix_of(&result.file, &interner);
        collect_file(
            &result.file,
            FileId(0),
            cone,
            &mut index,
            &mut interner,
            &mut diags,
        );
        index.resolve_extensions(&mut interner);
        let imports =
            ImportTable::collect(&result.file, FileId(0), &index, &mut interner, &mut diags);
        let mut env = TypeEnv::new(&index, &interner);
        crate::typecheck::env::register_top_level_signatures(
            &mut env,
            &result.file,
            &imports,
            &prefix,
            &mut diags,
        );
        crate::typecheck::env::register_members(
            &mut env,
            &result.file,
            &imports,
            &prefix,
            &mut diags,
        );
        crate::typecheck::env::register_constructors(
            &mut env,
            &result.file,
            &imports,
            &prefix,
            &mut diags,
        );
        let mut resolution = Resolution::new();
        crate::resolve::body::resolve_file_bodies(
            &result.file,
            &index,
            &imports,
            &interner,
            &mut diags,
            &mut resolution,
            &prefix,
        );
        // 找到 main 并检查其体。
        for item in &result.file.items {
            if let crate::syntax::ast::ItemKind::Fun(d) = &item.kind
                && interner.resolve(d.name.symbol) == "main"
                && let Some(body) = &d.body
            {
                let mut expr_types = crate::resolve::output::NodeIdTable::new();
                check_function(
                    &d.params,
                    d.return_ty.as_ref(),
                    body,
                    d.effect.as_ref(),
                    &mut env,
                    &imports,
                    &resolution,
                    &mut diags,
                    &prefix,
                    HashMap::new(),
                    HashSet::new(),
                    HashSet::new(),
                    None,
                    false,
                    &mut expr_types,
                    false,
                    false,
                );
            }
        }
        let codes = diags.iter().map(|d| d.code.to_string()).collect();
        (codes, interner)
    }

    fn main_returning(ret: &str, body: &str) -> String {
        format!("fun main(): {ret} {{\n{body}\n}}\n")
    }

    #[test]
    fn int_arithmetic_returns_int() {
        let (codes, _) = check(&main_returning("Int", "return 1 + 2 * 3"));
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn comparison_returns_bool_annotated() {
        let (codes, _) = check(&main_returning("Bool", "return 1 < 2"));
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn return_type_mismatch_on_int_bool_return() {
        let (codes, _) = check(&main_returning("Int", "return true"));
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::typecheck::return_type_mismatch"),
            "{codes:?}"
        );
    }

    #[test]
    fn local_val_inferred_and_used() {
        let (codes, _) = check(&main_returning("Int", "val x = 10\nreturn x + 1"));
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn if_expr_same_branch_types() {
        let (codes, _) = check(&main_returning(
            "Int",
            "val y = if (true) 1 else 2\nreturn y",
        ));
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn break_outside_loop_is_error() {
        let (codes, _) = check(&main_returning("Unit", "break"));
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::typecheck::break_not_in_loop"),
            "{codes:?}"
        );
    }

    #[test]
    fn out_of_scope_call_is_lenient() {
        let (codes, _) = check(&main_returning("Unit", "println(1)"));
        // println resolves via sysroot or returns Nothing (lenient).
        // No type_mismatch / arity_mismatch expected.
        assert!(
            codes.iter().all(|c| c != "scoop::typecheck::type_mismatch"
                && c != "scoop::typecheck::arity_mismatch"),
            "{codes:?}"
        );
    }

    #[test]
    fn while_body_checked() {
        let (codes, _) = check(&main_returning("Unit", "while (1 < 2) { val z = 5 }"));
        assert!(codes.is_empty(), "{codes:?}");
    }

    // ---- M2：调用 ----

    fn check_program(src: &str) -> Vec<String> {
        check(src).0
    }

    #[test]
    fn top_level_call_returns_int() {
        let codes = check_program(
            "fun inc(x: Int): Int { return x + 1 }\nfun main(): Int { return inc(5) }",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn call_arity_mismatch() {
        let codes = check_program("fun one(): Int { return 1 }\nfun main(): Int { return one(5) }");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::typecheck::call_arity_mismatch"),
            "{codes:?}"
        );
    }

    #[test]
    fn call_arg_type_mismatch() {
        let codes =
            check_program("fun id(x: Int): Int { return x }\nfun main(): Int { return id(true) }");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::typecheck::call_arg_type_mismatch"),
            "{codes:?}"
        );
    }

    #[test]
    fn member_access_on_struct_param() {
        let codes = check_program(
            "struct Point { val x: Int\nval y: Int }\nfun main(p: Point): Int { return p.x }",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    // ---- M3：多候选重载解析 ----

    #[test]
    fn overload_resolves_by_arg_type() {
        let codes = check_program(
            "fun f(x: Int): Int { return x }\nfun f(x: Bool): Bool { return x }\nfun main(): Int { return f(1) }",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn overload_no_applicable() {
        let codes = check_program(
            "fun f(x: Int): Int { return x }\nfun f(x: Bool): Bool { return x }\nfun main(): Unit { f(\"hi\") }",
        );
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::typecheck::no_applicable_overload"),
            "{codes:?}"
        );
    }

    // ---- M4：构造器调用 ----

    #[test]
    fn constructor_call_returns_nominal() {
        let codes = check_program(
            "struct Point(val x: Int, val y: Int)\nfun main(): Point { return Point(1, 2) }",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn struct_literal_field_check() {
        let codes = check_program(
            "struct Point(val x: Int, val y: Int)\nfun main(): Point { return Point { x: 1, y: 2 } }",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn tuple_literal_types() {
        let codes = check_program("fun main(): (Int, Bool) { return (1, true) }");
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn type_check_is_returns_bool() {
        let codes = check_program("fun main(): Bool { return 1 is Int }");
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn cast_as_returns_target() {
        let codes = check_program("fun main(): Int { return 1 as Int }");
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn lambda_typed_params_function_type() {
        let codes = check_program(
            "fun main(): Int { val f: (Int) -> Int = { x: Int -> x + 1 }\nreturn f(5) }",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn when_expr_lub_same_type() {
        let codes = check_program(
            "fun main(): Int { val v = 1\nval r = when (v) { 1 -> 2\nelse -> 3 }\nreturn r }",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn subtyping_interface_impl() {
        let codes = check_program(
            "interface Animal {}\nclass Dog : Animal {}\nfun main(): Animal { val d = Dog()\nreturn d }",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    // 抑制未用（TypeKind/Interner 在某些路径间接使用）。
    #[test]
    fn _ty_kind_compiles() {
        let _ = TypeKind::Nothing;
    }
}
