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

use std::collections::HashMap;

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{NodeId, Span, Symbol};

use crate::resolve::imports::ImportTable;
use crate::resolve::output::{Resolution, ResolvedValue};
use crate::syntax::ast::{
    self, AssignTargetKind, BinaryOp, Block, CallArg, Expr, ExprKind, FunBody, MemberName, Param,
    Stmt, StmtKind, TypeRef, UnaryOp, ValBinding,
};
use crate::ty::{FunctionType, TypeId, TypeKind, TypeParamType};

use super::diagnostics;
use super::env::{Signature, TypeEnv};
use super::lower::TypeLowering;

/// 检查一个函数体（M1+）。
#[allow(clippy::too_many_arguments)]
pub fn check_function<'a, 'i>(
    params: &[Param],
    return_ty: Option<&TypeRef>,
    body: &FunBody,
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
        return_ty: None,
        in_loop: 0u32,
        this_ty,
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
        FunBody::Block(b) => c.walk_block(b),
        FunBody::Expr(e) => {
            let t = c.walk_expr(e);
            if let Some(expected) = c.return_ty
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
    return_ty: Option<TypeId>,
    in_loop: u32,
    /// 成员函数体的 `this` 类型（lowered nominal）；非成员函数为 `None`。
    this_ty: Option<TypeId>,
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

    /// M1 赋值性：相等，或 `found` 为 `Nothing`。
    fn assignable(&self, found: TypeId, expected: TypeId) -> bool {
        found == expected || self.env.store.is_nothing(found)
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
        self.diags
            .push(diagnostics::unsupported_in_this_phase(what, span));
        self.env.store.nothing()
    }

    // ---- 语句 / 块 ----

    fn walk_block(&mut self, block: &Block) {
        for s in &block.stmts {
            self.walk_stmt(s);
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
                        if let Some(&expected) = self.locals.get(&ident.symbol) {
                            self.check_assignable(val_ty, expected, value.span);
                        } else {
                            // 非局部赋值目标（成员 / 顶层）→ M2+。
                            self.unsupported("非局部赋值", target.span);
                        }
                    }
                    AssignTargetKind::Member { .. } | AssignTargetKind::Index { .. } => {
                        self.unsupported("成员 / 下标赋值", target.span);
                    }
                }
            }
            StmtKind::LocalVal(val) => self.walk_local_val(val, stmt),
            StmtKind::Return { value } => {
                if let Some(e) = value {
                    let t = self.walk_expr(e);
                    if let Some(expected) = self.return_ty {
                        self.check_assignable(t, expected, e.span);
                    }
                }
            }
            StmtKind::While { cond, body } => {
                let ct = self.walk_expr(cond);
                let bool_ty = self.env.store.bool();
                self.check_assignable(ct, bool_ty, cond.span);
                self.in_loop += 1;
                self.walk_block(body);
                self.in_loop -= 1;
            }
            StmtKind::For { .. } => {
                self.unsupported("for 循环", stmt.span);
            }
            StmtKind::Break => {
                if self.in_loop == 0 {
                    self.diags
                        .push(diagnostics::break_not_in_loop("break", stmt.span));
                }
            }
            StmtKind::Continue => {
                if self.in_loop == 0 {
                    self.diags
                        .push(diagnostics::break_not_in_loop("continue", stmt.span));
                }
            }
        }
    }

    fn walk_local_val(&mut self, val: &ast::ValDecl, stmt: &Stmt) {
        let declared = val.ty.as_ref().map(|t| self.lower_type(t));
        let init_ty = val.init.as_ref().map(|e| self.walk_expr(e));
        if let (Some(d), Some(i)) = (declared, init_ty) {
            self.check_assignable(i, d, stmt.span);
        }
        let ty = declared
            .or(init_ty)
            .unwrap_or_else(|| self.env.store.nothing());
        match &val.binding {
            ValBinding::Name(name) => {
                self.locals.insert(name.symbol, ty);
            }
            ValBinding::Pattern(_) => {
                self.unsupported("解构绑定", stmt.span);
            }
        }
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
                    return t;
                }
                // 顶层值 / 函数引用 / 类型名 → M2+。
                self.unsupported("非局部名字引用", expr.span)
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
                    None => self.unsupported("该运算符 / 操作数组合", expr.span),
                }
            }
            ExprKind::Unary { op, expr: e } => {
                let t = self.walk_expr(e);
                let cat = scalar_cat(self.env.store.kind(t));
                let bool_ty = self.env.store.bool();
                match builtin_unary(*op, cat, t, bool_ty) {
                    Some(t) => t,
                    None => self.unsupported("该一元运算", expr.span),
                }
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let ct = self.walk_expr(cond);
                let bool_ty = self.env.store.bool();
                self.check_assignable(ct, bool_ty, cond.span);
                let tt = self.walk_expr(then_branch);
                let et = match else_branch {
                    Some(e) => self.walk_expr(e),
                    None => self.env.store.unit(),
                };
                // M1 LUB：两分支同类型 → 该类型；否则 Unit（精确 LUB 在后续里程碑）。
                if tt == et { tt } else { self.env.store.unit() }
            }
            ExprKind::Block(b)
            | ExprKind::DoBlock(b)
            | ExprKind::UnsafeBlock(b)
            | ExprKind::SafeBlock(b) => {
                self.walk_block(b);
                self.env.store.unit()
            }
            ExprKind::Call { callee, args } => self.type_call(callee, args, expr.span),
            ExprKind::MemberAccess { receiver, member } => {
                let rt = self.walk_expr(receiver);
                self.member_access_type(rt, *member, expr.span)
            }
            // 其余形式属于后续里程碑。
            ExprKind::TupleLit(_)
            | ExprKind::ArrayLit(_)
            | ExprKind::StructLit { .. }
            | ExprKind::Lambda(_)
            | ExprKind::When { .. }
            | ExprKind::Handle { .. }
            | ExprKind::SafeMemberAccess { .. }
            | ExprKind::SpliceField { .. }
            | ExprKind::Index { .. }
            | ExprKind::NotNullAssert { .. }
            | ExprKind::TypeApply { .. }
            | ExprKind::ClassLit { .. }
            | ExprKind::InfixCall { .. }
            | ExprKind::TypeCheck { .. }
            | ExprKind::Cast { .. }
            | ExprKind::WithUpdate { .. }
            | ExprKind::Annotated { .. } => self.unsupported("该表达式形式", expr.span),
        }
    }

    // ---- 调用（M2 单候选 + M3 多候选重载解析） ----

    fn type_call(&mut self, callee: &Expr, args: &[CallArg], span: Span) -> TypeId {
        // 1. 函数类型局部值 / 参数调用。
        if let ExprKind::Ident(ident) = &callee.kind
            && let Some(&ft) = self.locals.get(&ident.symbol)
            && let Some(f) = self.function_type_of(ft)
        {
            return self.check_call_args(f.params, f.return_ty, args, span);
        }
        // 2. 顶层函数调用（resolve 解析到 TopLevelFun）。
        if let ExprKind::Ident(_) = &callee.kind {
            let resolved = self.resolution.value_refs.get(callee.id).copied();
            if let Some(ResolvedValue::TopLevelFun { fqn }) = resolved {
                return self.resolve_top_level_call(fqn, args, span);
            }
        }
        self.unsupported("该调用形式", span)
    }

    /// 顶层函数重载解析：单候选 → 直接检查（arity / 实参）；多候选 → 适用性过滤。
    fn resolve_top_level_call(&mut self, fqn: Symbol, args: &[CallArg], span: Span) -> TypeId {
        let sigs: Vec<Signature> = self.env.signatures(fqn).unwrap_or(&[]).to_vec();
        let non_generic: Vec<Signature> = sigs
            .into_iter()
            .filter(|s| s.type_param_count == 0)
            .collect();
        if non_generic.is_empty() {
            return self.unsupported("该调用（仅泛型候选；泛型实例化待 M3 part 2）", span);
        }
        // 单候选：直接检查（保留 arity_mismatch / type_mismatch 精确诊断）。
        if non_generic.len() == 1 {
            let sig = non_generic.into_iter().next().expect("len == 1");
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
                self.diags.push(diagnostics::no_applicable_overload(span));
                non_generic
                    .first()
                    .map(|s| s.return_ty)
                    .unwrap_or_else(|| self.env.store.nothing())
            }
            1 => applicable[0].return_ty,
            _ => {
                self.diags.push(diagnostics::ambiguous_overload(span));
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
        if param_types.len() != args.len() {
            self.diags.push(diagnostics::arity_mismatch(
                param_types.len(),
                args.len(),
                span,
            ));
            for a in args {
                self.walk_expr(&a.value);
            }
            return return_ty;
        }
        for (i, a) in args.iter().enumerate() {
            let at = self.walk_expr(&a.value);
            self.check_assignable(at, param_types[i], a.value.span);
        }
        return_ty
    }

    // ---- 成员访问（M2 part 2：属性 / 字段读取） ----

    fn member_access_type(
        &mut self,
        receiver_ty: TypeId,
        member: MemberName,
        span: Span,
    ) -> TypeId {
        let MemberName::Named(name) = member else {
            return self.unsupported("元组下标成员访问", span);
        };
        let Some(fqn) = nominal_fqn_of(self.env.store.kind(receiver_ty)) else {
            return self.unsupported("该接收者的成员访问", span);
        };
        match self.env.member_type(fqn, name.symbol) {
            Some(t) => t,
            None => self.unsupported("该成员（方法调用 / 未注册成员）", span),
        }
    }
}

/// 若 `kind` 是 nominal（ref 或 value），返回其 FQN。
fn nominal_fqn_of(kind: &TypeKind) -> Option<Symbol> {
    match kind {
        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
        | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(n.fqn),
        _ => None,
    }
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
    fn type_mismatch_on_int_bool_return() {
        let (codes, _) = check(&main_returning("Int", "return true"));
        assert!(
            codes.iter().any(|c| c == "scoop::typecheck::type_mismatch"),
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
    fn out_of_scope_call_is_unsupported() {
        let (codes, _) = check(&main_returning("Unit", "println(1)"));
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::typecheck::unsupported_in_this_phase"),
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
                .any(|c| c == "scoop::typecheck::arity_mismatch"),
            "{codes:?}"
        );
    }

    #[test]
    fn call_arg_type_mismatch() {
        let codes =
            check_program("fun id(x: Int): Int { return x }\nfun main(): Int { return id(true) }");
        assert!(
            codes.iter().any(|c| c == "scoop::typecheck::type_mismatch"),
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

    // 抑制未用（TypeKind/Interner 在某些路径间接使用）。
    #[test]
    fn _ty_kind_compiles() {
        let _ = TypeKind::Nothing;
    }
}
