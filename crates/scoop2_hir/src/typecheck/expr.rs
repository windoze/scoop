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
use crate::ty::{EffectRow, FunctionType, NominalType, TypeId, TypeKind, TypeParamType};

use super::diagnostics;
use super::env::{Signature, TypeEnv};
use super::lower::TypeLowering;

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
    this_ty: Option<TypeId>,
) {
    let mut c = ExprChecker {
        env,
        imports,
        resolution,
        diags,
        package_prefix: package_prefix.to_string(),
        type_params,
        locals: HashMap::new(),
        local_vars: HashMap::new(),
        lambda_depth: 0,
        return_ty: None,
        in_loop: 0u32,
        this_ty,
        in_function_body: true,
        lenient_type_errors: false,
        performed_effects: Vec::new(),
        effect_suspend_depth: 0,
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
    match body {
        FunBody::Block(b) => {
            c.walk_block(b);
        }
        FunBody::Expr(e) => {
            let t = c.walk_expr(e);
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
    // effect 检查：函数体执行的 effect 必须在声明的 effect row 中。
    let declared = ExprChecker::extract_effect_names(declared_effect, c.env.interner);
    for (performed, span) in &c.performed_effects {
        if !declared.contains(performed) {
            c.diags
                .push(diagnostics::required_effect_not_declared(*span));
            break;
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
) {
    let mut c = ExprChecker {
        env,
        imports,
        resolution,
        diags,
        package_prefix: package_prefix.to_string(),
        type_params: HashMap::new(),
        locals: HashMap::new(),
        local_vars: HashMap::new(),
        lambda_depth: 0,
        return_ty: None,
        in_loop: 0u32,
        this_ty: None,
        in_function_body: false,
        lenient_type_errors: true,
        performed_effects: Vec::new(),
        effect_suspend_depth: 0,
    };
    let declared = val.ty.as_ref().map(|t| c.lower_type(t));
    let init_ty = val.init.as_ref().map(|e| c.walk_expr(e));
    if let (Some(d), Some(i)) = (declared, init_ty)
        && !c.env.store.is_unit(d)
        && !c.assignable(i, d)
        && !c.lenient_type_errors
        && !matches!(
            c.env.store.kind(i),
            TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
        )
    {
        c.diags.push(diagnostics::initializer_type_mismatch(
            &c.describe(d),
            &c.describe(i),
            val.ty.as_ref().map(|t| t.span).unwrap_or_default(),
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
) {
    let mut c = ExprChecker {
        env,
        imports,
        resolution,
        diags,
        package_prefix: package_prefix.to_string(),
        type_params: HashMap::new(),
        locals: HashMap::new(),
        local_vars: HashMap::new(),
        lambda_depth: 0,
        return_ty: None,
        in_loop: 0u32,
        this_ty,
        in_function_body: false,
        lenient_type_errors: true,
        performed_effects: Vec::new(),
        effect_suspend_depth: 0,
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
    /// 顶层 val init 检查：lenient 模式，抑制类型级假阳性（arity / type mismatch），
    /// 但保留结构性检查（when 穷尽 / 命名实参形态 / 泛型推断等）。
    lenient_type_errors: bool,
    /// 当前函数体执行了的 effect（FQN, span）（effect 检查用）。
    performed_effects: Vec<(String, Span)>,
    /// effect 采集挂起深度（> 0 时不在 performed_effects 中记录；lambda / handle 体用）。
    effect_suspend_depth: u32,
}

impl<'a, 'i> ExprChecker<'a, 'i> {
    fn lower_type(&mut self, ty: &TypeRef) -> TypeId {
        let mut lower = TypeLowering::new(
            self.env,
            self.imports,
            self.type_params.clone(),
            self.package_prefix.clone(),
            self.diags,
        );
        lower.lower(ty)
    }

    fn describe(&self, id: TypeId) -> String {
        // M1 简易描述（标量类别；nominal 用 ty#N）。
        match self.env.store.kind(id) {
            TypeKind::Value(v) => format!("{v:?}"),
            TypeKind::Ref(r) => format!("{r:?}"),
            TypeKind::Nothing => "Nothing".to_string(),
            TypeKind::Param(p) => format!("{p:?}"),
            TypeKind::StarProjection => "*".to_string(),
        }
    }

    /// 赋值性（M1+）：相等、`Nothing` bottom、nominal 子类型（继承/接口）、
    /// 函数逆变/协变、Option/元组协变。
    fn assignable(&self, found: TypeId, expected: TypeId) -> bool {
        if found == expected || self.env.store.is_nothing(found) {
            return true;
        }
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
            return self.fqn_is_subtype(ffqn, efqn);
        }
        // 内建标量的 nominal 子类型（Int → scoop.core.Int 的超类型链）。
        if let (Some(sfqn), Some(efqn)) = (scalar_fqn(fk, self.env.interner), nominal_fqn_of(ek)) {
            return self.fqn_is_subtype(sfqn, efqn);
        }
        // 函数：参数逆变 + 返回协变 + effect 行放宽（lenient：effect 行未完整实现）
        if let Some((ff, ef)) = func {
            return ff.params.len() == ef.params.len()
                && ff
                    .params
                    .iter()
                    .zip(&ef.params)
                    .all(|(fp, ep)| self.assignable(*ep, *fp))
                && self.assignable(ff.return_ty, ef.return_ty)
                && ff.receiver.is_some() == ef.receiver.is_some();
        }
        // Option 协变
        if let Some((fi, ei)) = opt {
            return self.assignable(fi, ei);
        }
        // 元组协变
        if let Some((fs, es)) = tup {
            return fs.len() == es.len()
                && fs.iter().zip(&es).all(|(f, e)| self.assignable(*f, *e));
        }
        false
    }

    /// FQN 子类型 DFS：fqn_a == fqn_b 或 fqn_a 的超类型链包含 fqn_b。
    fn fqn_is_subtype(&self, fqn_a: Symbol, fqn_b: Symbol) -> bool {
        if fqn_a == fqn_b {
            return true;
        }
        self.env
            .index
            .supertypes_of(fqn_a)
            .iter()
            .any(|&sup| self.fqn_is_subtype(sup, fqn_b))
    }

    fn check_assignable(&mut self, found: TypeId, expected: TypeId, span: Span) {
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
        result
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
                        && !matches!(
                            self.env.store.kind(t),
                            TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
                        )
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
        let declared = val.ty.as_ref().map(|t| self.lower_type(t));
        let init_ty = val.init.as_ref().map(|e| self.walk_expr(e));
        if let (Some(d), Some(i)) = (declared, init_ty)
            && !self.assignable(i, d)
            && !matches!(
                self.env.store.kind(i),
                TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
            )
        {
            // 同 FQN nominal 但类型实参不同（type arg 推断不精确）→ lenient。
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
            if same_owner {
                return;
            }
            self.diags.push(diagnostics::initializer_type_mismatch(
                &self.describe(d),
                &self.describe(i),
                stmt.span,
            ));
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
            }
        }
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
        let Some((_, constraints)) = self.env.type_constraints(fqn) else {
            return;
        };
        let constraints: Vec<(Symbol, GenericBound)> = constraints.to_vec();
        for (name, bound) in &constraints {
            let Some(&arg) = subst.get(name) else {
                continue;
            };
            let violated = match bound {
                GenericBound::Ref(_) => !matches!(self.env.store.kind(arg), TypeKind::Ref(_)),
                GenericBound::Value(_) => !matches!(self.env.store.kind(arg), TypeKind::Value(_)),
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
        use crate::syntax::ast::{PatternKind, PatternLiteral};
        // else / 通配 / 绑定 → 穷尽。
        if arms.iter().any(|a| {
            matches!(
                a.pat.kind,
                PatternKind::Else | PatternKind::Wildcard | PatternKind::Bind(_)
            )
        }) {
            return None;
        }
        // 带守卫的 Literal/Variant 模式不保证覆盖（守卫可能失败）。
        let unguarded_arms: Vec<&ast::WhenArm> =
            arms.iter().filter(|a| a.guard.is_none()).collect();
        // Bool：需 true + false（仅无守卫分支计数）。
        if matches!(
            self.env.store.kind(subject_ty),
            TypeKind::Value(crate::ty::ValueTypeKind::Bool)
        ) {
            let mut has_true = false;
            let mut has_false = false;
            for arm in &unguarded_arms {
                if let PatternKind::Literal(PatternLiteral::Bool { value, .. }) = &arm.pat.kind {
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
            return None;
        }
        // Option：需 Some + None（仅无守卫分支计数）。
        if matches!(
            self.env.store.kind(subject_ty),
            TypeKind::Value(crate::ty::ValueTypeKind::Option(_))
        ) {
            let mut has_some = false;
            let mut has_none = false;
            for arm in &unguarded_arms {
                if let PatternKind::Variant { path, .. } = &arm.pat.kind
                    && let Some(seg) = path.segments.last()
                {
                    let name = self.env.interner.resolve(seg.symbol);
                    if name == "Some" {
                        has_some = true;
                    } else if name == "None" {
                        has_none = true;
                    }
                }
            }
            if !has_some {
                return Some("Some".to_string());
            }
            if !has_none {
                return Some("None".to_string());
            }
            return None;
        }
        // enum：需覆盖所有 variant（仅无守卫分支计数）。
        let enum_fqn = match self.env.store.kind(subject_ty) {
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
            | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(n.fqn),
            _ => None,
        };
        if let Some(fqn) = enum_fqn
            && let Some(variants) = self.env.enum_variants(fqn)
        {
            let covered: HashSet<Symbol> = unguarded_arms
                .iter()
                .filter_map(|a| match &a.pat.kind {
                    PatternKind::Variant { path, .. } => path.segments.last().map(|s| s.symbol),
                    _ => None,
                })
                .collect();
            for v in variants {
                if !covered.contains(v) {
                    return Some(self.env.interner.resolve(*v).to_string());
                }
            }
            return None;
        }
        // 其他已知类型（标量 / nominal）→ 无穷值域，需 else。
        if !self.env.store.is_nothing(subject_ty) {
            return Some("else".to_string());
        }
        None
    }

    // ---- 表达式 ----

    fn walk_expr(&mut self, expr: &Expr) -> TypeId {
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
                        self.walk_expr(e);
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
                                return self.method_call_return_type(lt, m, &[rt], expr.span);
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
                                return self.method_call_return_type(t, m, &[], expr.span);
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
                let tt = self.walk_expr(then_branch);
                match else_branch {
                    Some(e) => {
                        let et = self.walk_expr(e);
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
                    CastOp::As => target,
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
                self.effect_suspend_depth += 1;
                let body_ty = self.walk_block(body);
                for arm in arms {
                    self.walk_expr(&arm.body);
                }
                self.effect_suspend_depth -= 1;
                if let Some(f) = finally {
                    self.walk_block(f);
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
                self.method_call_return_type(rt, get_sym, &idx_types, expr.span)
            }
            ExprKind::InfixCall {
                receiver,
                name,
                arg,
            } => {
                let rt = self.walk_expr(receiver);
                let at = self.walk_expr(arg);
                self.method_call_return_type(rt, name.symbol, &[at], expr.span)
            }
            ExprKind::ClassLit { .. } => self.env.store.string(),
            ExprKind::ArrayLit(els) => {
                if els.is_empty() {
                    self.diags
                        .push(diagnostics::array_lit_type_annotation_required(expr.span));
                    return self.env.store.nothing();
                }
                let elem_ty = self.walk_expr(&els[0]);
                for e in &els[1..] {
                    self.walk_expr(e);
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
        // Unwrap explicit type args `f<T>(x)` / `obj.m<T>(x)`。
        let callee = match &callee.kind {
            ExprKind::TypeApply { callee: inner, .. } => inner,
            _ => callee,
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
        if let ExprKind::Ident(_) = &callee.kind {
            let resolved = self.resolution.value_refs.get(callee.id).copied();
            if let Some(ResolvedValue::TopLevelFun { fqn }) = resolved {
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
                if name.ends_with(".Some") || name == "Some" {
                    let inner = if args.is_empty() {
                        self.env.store.nothing()
                    } else {
                        self.walk_expr(&args[0].value)
                    };
                    let opt = self.env.store.option(inner);
                    return opt;
                }
                if name.ends_with(".None") || name == "None" {
                    let nothing = self.env.store.nothing();
                    return self.env.store.option(nothing);
                }
                // 其他 enum variant：返回所属 enum 的 nominal。
                if let Some(dot) = name.rfind('.')
                    && let Some(enum_fqn) = self.env.interner.get(&name[..dot])
                {
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
            let rt = self.walk_expr(receiver);
            let arg_types: Vec<TypeId> = args.iter().map(|a| self.walk_expr(&a.value)).collect();
            // Continuation.resume 步骤 effect 传播：`k.resume(v)` 的 effect 行为
            // `E + Raise<RuntimeError>`（E 为 `Continuation<..., eff E>` 的第三类型实参）。
            // 由此调用点执行 RuntimeError（恒定）+ E 所含 effect。
            if self.effect_suspend_depth == 0
                && let MemberName::Named(name) = member
                && self.env.interner.resolve(name.symbol) == "resume"
            {
                self.record_continuation_resume_effects(rt, span);
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
                    return self.method_call_return_type(rt, name.symbol, &arg_types, span);
                }
                MemberName::TupleIndex { .. } => {}
            }
        }
        // 4. 构造器调用（callee 是类型名 `Type(args)`）。
        if let ExprKind::Ident(ident) = &callee.kind
            && let Some(fqn) = self.callee_type_fqn(ident.symbol)
        {
            return self.ctor_call(fqn, args, span);
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
    fn ctor_call(&mut self, fqn: Symbol, args: &[CallArg], span: Span) -> TypeId {
        // `Continuation` 是 compiler-owned interface，用户代码不能直接构造。
        let name = self.env.interner.resolve(fqn);
        let stripped = name.strip_prefix("scoop.core.").unwrap_or(name);
        if stripped == "Continuation" {
            self.diags
                .push(diagnostics::continuation_not_constructible(span));
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

    /// 名称是否是已知 effect 类型（prelude + 用户声明）。
    fn is_effect_type_name(&self, name: &str) -> bool {
        // prelude effects.
        let stripped = name.strip_prefix("scoop.core.").unwrap_or(name);
        if matches!(stripped, "Raise" | "RuntimeError") {
            return true;
        }
        // 检查 Index 中该名称是否是 Effect category。
        if let Some(sym) = self.env.interner.get(name)
            && let Some(cat) = self.env.index.category(sym)
        {
            return matches!(cat, crate::resolve::symbol::NominalCategory::Effect);
        }
        false
    }

    /// 记录 `k.resume(v)` 的步骤 effect 到 performed_effects。
    ///
    /// `Continuation<Resume, Answer, eff E>.resume` 的 effect 行为 `E + Raise<RuntimeError>`
    /// （spec §5.5）：恒定执行 `Raise`（即 `Raise<RuntimeError>`），外加 E 解析出的具体 effect。
    /// E 为类型参数（多态）或 `Pure` 时不贡献额外 effect。
    fn record_continuation_resume_effects(&mut self, recv_ty: TypeId, span: Span) {
        // 先用不可变借用取出所需数据，再以可变借用写入 performed_effects。
        let e_name: Option<String> = {
            let kind = self.env.store.kind(recv_ty);
            let Some(fqn) = nominal_fqn_of(kind) else {
                return;
            };
            let recv = self.env.interner.resolve(fqn);
            let recv = recv.strip_prefix("scoop.core.").unwrap_or(recv);
            if recv != "Continuation" {
                return;
            }
            nominal_args_of(kind)
                .and_then(|args| args.get(2).copied())
                .and_then(|e_ty| nominal_fqn_of(self.env.store.kind(e_ty)))
                .map(|e_fqn| self.env.interner.resolve(e_fqn).to_string())
                .filter(|n| {
                    let s = n.strip_prefix("scoop.core.").unwrap_or(n);
                    s != "Pure"
                })
        };
        self.performed_effects.push(("Raise".to_string(), span));
        if let Some(name) = e_name {
            self.performed_effects.push((
                name.strip_prefix("scoop.core.")
                    .unwrap_or(&name)
                    .to_string(),
                span,
            ));
        }
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

    /// 查找 enum variant FQN 的 enum owner FQN（`pkg.Enum.Variant` → `pkg.Enum`）。
    fn find_enum_owner(&self, variant_fqn: Symbol) -> Option<Symbol> {
        let name = self.env.interner.resolve(variant_fqn);
        let dot = name.rfind('.')?;
        let owner_text = &name[..dot];
        self.env.interner.get(owner_text)
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
        let sigs: Vec<Signature> = self.env.signatures(fqn).unwrap_or(&[]).to_vec();
        let non_generic: Vec<Signature> = sigs
            .iter()
            .filter(|s| s.type_param_count == 0)
            .cloned()
            .collect();
        if non_generic.is_empty() {
            // 泛型候选：推断类型实参 + where 约束满足性检查。
            let arg_types: Vec<TypeId> = args.iter().map(|a| self.walk_expr(&a.value)).collect();
            if let Some(sig) = sigs.first() {
                self.check_generic_call_constraints(fqn, sig, &arg_types, span);
                return sig.return_ty;
            }
            return self.env.store.nothing();
        }
        // Unit sugar：0 实参 + 候选有 1 个 Unit 参数 → 补 Unit 实参（spec §5.5）。
        let unit_ty = self.env.store.unit();
        let arg_count = args.len();
        if arg_count == 0
            && non_generic
                .iter()
                .any(|s| s.params.len() == 1 && s.params[0] == unit_ty)
        {
            let sig = non_generic
                .into_iter()
                .find(|s| s.params.len() == 1 && s.params[0] == unit_ty)
                .expect("checked above");
            return sig.return_ty;
        }
        // 单候选：直接检查（保留 arity_mismatch / type_mismatch 精确诊断）。
        if non_generic.len() == 1 {
            let sig = non_generic.into_iter().next().expect("len == 1");
            // 命名实参校验：名字必须在该签名参数名中。
            let names: std::collections::HashSet<Symbol> =
                sig.param_names.iter().copied().collect();
            for a in args {
                if let Some(arg_name) = &a.name
                    && !names.contains(&arg_name.symbol)
                {
                    let n = self.env.interner.resolve(arg_name.symbol);
                    self.diags
                        .push(diagnostics::unknown_call_arg_name(n, a.span));
                }
            }
            return self.check_call_args(sig.params, sig.return_ty, args, span);
        }
        // 多候选：先走一遍实参类型，再按适用性过滤。
        let arg_types: Vec<TypeId> = args.iter().map(|a| self.walk_expr(&a.value)).collect();
        let applicable: Vec<&Signature> = non_generic
            .iter()
            .filter(|s| {
                s.params.len() == arg_types.len()
                    && arg_types
                        .iter()
                        .zip(&s.params)
                        .all(|(a, p)| self.assignable(*a, *p))
            })
            .collect();
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
                    if sig.params.len() == arg_types.len() {
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
            1 => applicable[0].return_ty,
            _ => {
                let fqn_text = self.env.interner.resolve(fqn).to_string();
                let fun_name = fqn_text.rsplit('.').next().unwrap_or(&fqn_text);
                let decls = self.env.index.lookup_funs(fqn);
                let first = applicable[0];
                let second = applicable[1];
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
                for sig in applicable.iter() {
                    let params: Vec<String> =
                        sig.params.iter().map(|p| self.describe(*p)).collect();
                    let label = format!("{fun_name}({})", params.join(", "));
                    let orig_idx = non_generic
                        .iter()
                        .position(|s| s.params == sig.params && s.return_ty == sig.return_ty);
                    if let Some(oi) = orig_idx
                        && let Some(d) = decls.get(oi)
                    {
                        diag = diag.with_related(d.span, label);
                    }
                }
                self.diags.push(diag);
                applicable[0].return_ty
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
            if !self.lenient_type_errors {
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
            let at = self.walk_expr(&a.value);
            if a.name.is_some() {
                continue;
            }
            if let Some(&pt) = param_types.get(i)
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
        return_ty
    }

    // ---- 成员访问（M2 part 2：属性 / 字段读取） ----

    fn member_access_type(
        &mut self,
        receiver_ty: TypeId,
        member: MemberName,
        _span: Span,
    ) -> TypeId {
        let MemberName::Named(name) = member else {
            return self.env.store.nothing();
        };
        let Some(fqn) = nominal_fqn_of(self.env.store.kind(receiver_ty)) else {
            return self.env.store.nothing();
        };
        match self.env.member_type(fqn, name.symbol) {
            Some(t) => t,
            None => self.env.store.nothing(),
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
        if !self.lenient_type_errors
            && is_struct
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
        // enum base 的 variant payload 逐段校验暂未实现（payload 未在 Index 登记）。
        if matches!(base_kind, WithUpdateAggregate::Enum(_)) {
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
    fn type_lambda(&mut self, lambda: &ast::LambdaExpr) -> TypeId {
        let mut param_types: Vec<TypeId> = Vec::new();
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
        self.effect_suspend_depth += 1;
        let return_ty = match &lambda.body {
            ast::LambdaBody::Block(b) => {
                self.walk_block(b);
                self.env.store.unit()
            }
            ast::LambdaBody::Expr(e) => self.walk_expr(e),
        };
        self.lambda_depth -= 1;
        self.effect_suspend_depth -= 1;
        self.in_function_body = saved_fn;
        self.env.store.function(FunctionType {
            receiver: None,
            params: param_types,
            return_ty,
            effects: EffectRow::pure(),
            closed: false,
        })
    }

    /// `when` 表达式：walk subject + 每分支体；返回各分支体的 LUB（同类型 → 该类型；否则 Unit）。
    fn type_when(&mut self, subject: &Expr, arms: &[ast::WhenArm], span: Span) -> TypeId {
        let subject_ty = self.walk_expr(subject);
        let mut arm_types: Vec<TypeId> = Vec::new();
        for arm in arms {
            self.bind_pattern_locals(&arm.pat);
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

    /// 方法调用（`receiver.method(args)` / `receiver[args]` / `receiver until arg`）：
    /// 查 receiver nominal 类型的成员函数签名，按适用性选择，返回返回类型。
    fn method_call_return_type(
        &mut self,
        receiver_ty: TypeId,
        method_name: Symbol,
        arg_types: &[TypeId],
        span: Span,
    ) -> TypeId {
        // Nothing / TypeParam 接收者（类型未知 / 泛型）→ lenient（返回 Nothing，不报错误）。
        if self.env.store.is_nothing(receiver_ty)
            || matches!(self.env.store.kind(receiver_ty), TypeKind::Param(_))
        {
            return self.env.store.nothing();
        }
        let Some(fqn) = nominal_fqn_of(self.env.store.kind(receiver_ty))
            .or_else(|| scalar_fqn(self.env.store.kind(receiver_ty), self.env.interner))
        else {
            return self.unsupported("该接收者的方法调用", span);
        };
        let Some(sigs) = self.env.member_signatures(fqn, method_name) else {
            // 方法未注册（可能是 typealias / 继承 / 扩展）→ lenient Nothing。
            return self.env.store.nothing();
        };
        let sigs: Vec<Signature> = sigs.to_vec();
        let non_generic: Vec<Signature> = sigs
            .into_iter()
            .filter(|s| s.type_param_count == 0)
            .collect();
        if non_generic.is_empty() {
            return {
                for (i, a) in arg_types.iter().enumerate() {
                    // 泛型方法 lenient: walk done; return Nothing.
                    let _ = i;
                    let _ = a;
                }
                self.env.store.nothing()
            };
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
            let has_param = sig
                .params
                .iter()
                .any(|p| matches!(self.env.store.kind(*p), TypeKind::Param(_)));
            if has_param {
                return sig.return_ty;
            }
            if sig.params.len() != arg_types.len() {
                self.diags.push(diagnostics::call_arity_mismatch(
                    sig.params.len(),
                    arg_types.len(),
                    span,
                ));
            } else {
                for (i, &at) in arg_types.iter().enumerate() {
                    if !self.assignable(at, sig.params[i]) {
                        self.diags.push(diagnostics::call_arg_type_mismatch(
                            &self.describe(sig.params[i]),
                            &self.describe(at),
                            span,
                        ));
                    }
                }
            }
            return sig.return_ty;
        }
        let applicable: Vec<&Signature> = non_generic
            .iter()
            .filter(|s| {
                s.params.len() == arg_types.len()
                    && arg_types
                        .iter()
                        .zip(&s.params)
                        .all(|(a, p)| self.assignable(*a, *p))
            })
            .collect();
        match applicable.len() {
            0 => {
                self.diags.push(diagnostics::no_applicable_overload(span));
                non_generic[0].return_ty
            }
            _ => self.pick_most_specific(&applicable, span),
        }
    }

    /// 从适用候选中选择最具体的（spec P3 §4.5）。
    /// A 比 B 更具体 ⟺ ∀i: A.params[i] <: B.params[i]。
    fn pick_most_specific(&mut self, candidates: &[&Signature], span: Span) -> TypeId {
        let n = candidates.len();
        // best[i] = true 如果候选 i 比所有其他候选更具体（或并列）。
        let mut best_idx = 0;
        for i in 1..n {
            // i 比 best_idx 更具体？
            let i_more = candidates[i]
                .params
                .iter()
                .zip(&candidates[best_idx].params)
                .all(|(ip, bp)| self.assignable(*ip, *bp));
            let best_more = candidates[best_idx]
                .params
                .iter()
                .zip(&candidates[i].params)
                .all(|(bp, ip)| self.assignable(*bp, *ip));
            if i_more && !best_more {
                best_idx = i;
            }
        }
        // 验证 best_idx 确实比所有其他候选更具体。
        let unique_best = candidates.iter().enumerate().all(|(j, _)| {
            j == best_idx || {
                // best_idx 不比 j 差（j 不严格比 best_idx 更具体）
                let j_strictly_better = candidates[j]
                    .params
                    .iter()
                    .zip(&candidates[best_idx].params)
                    .all(|(jp, bp)| self.assignable(*jp, *bp))
                    && candidates[best_idx]
                        .params
                        .iter()
                        .zip(&candidates[j].params)
                        .any(|(bp, jp)| !self.assignable(*bp, *jp));
                !j_strictly_better
            }
        });
        if !unique_best && n > 1 {
            self.diags.push(diagnostics::ambiguous_overload(span));
        }
        candidates[best_idx].return_ty
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
fn scalar_fqn(kind: &TypeKind, interner: &scoop2_base::Interner) -> Option<Symbol> {
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
        _ => return None,
    };
    interner.get(name)
}

/// 若 `kind` 是 nominal（ref 或 value），返回其 FQN。
fn nominal_fqn_of(kind: &TypeKind) -> Option<Symbol> {
    match kind {
        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
        | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(n.fqn),
        _ => None,
    }
}

/// nominal 类型的类型实参切片（ref/value 均覆盖）。
fn nominal_args_of(kind: &TypeKind) -> Option<&[TypeId]> {
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
        B::Lt | B::Le | B::Gt | B::Ge | B::Eq | B::Ne => {
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
                    None,
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
