//! AST desugar（reshape）pass：把语法糖改写成基础结构。
//!
//! 这是 **pure-function** 变换：自底向上递归，输入一个 AST 节点 + span，输出一个
//! reshape 后的 AST 节点。不做任何 typecheck / resolve——所有语义错误留给后续阶段
//! 统一报错。唯一可变状态是 `&mut NodeIdAllocator`（给合成节点分配 id）。
//!
//! 挂载点：parse 之后、typecheck 之前（driver 调 [`reshape_file`]）。allocator 从
//! parse 产物的 `node_count` 接着编号，保证合成 NodeId 不与既有冲突。
//!
//! ## 当前覆盖的 desugar 项
//!
//! - `T?` → `Option<T>`（类型层 [`reshape_type`]）
//!
//! ## 待加（按依赖序）
//!
//! - operator(`+`/`<`/...) → method call
//! - `a[i]` → `a.get(i)`、`a[i] = v` → `a.set(i, v)`
//! - `a!!` → `a.unwrap()`
//! - `a?.b` → `when(a){Some(v)->v.b; None->None}`
//! - `f"..."` → `StringBuilder().add(...).build()`
//! - `for (x in xs) BODY` → `do{var __it=xs.iterator(); while(true){when(__it.next()){Some(x)->BODY; None->break}}}`

use scoop2_base::{Interner, NodeId, NodeIdAllocator, Span, Symbol};

use crate::ast::{
    decl::{FunBody, FunDecl, Item, ItemKind, TypeAliasDecl, TypeDecl, ValDecl},
    expr::{
        AssignTarget, AssignTargetKind, Block, CallArg, Expr, ExprKind, LambdaExpr, MemberName,
        Stmt, StmtKind, WhenArm,
    },
    pattern::{Pattern, PatternKind},
    types::{TypeArg, TypeArgKind, TypeRef, TypeRefKind},
    Ident, TypePath,
};

// ===========================================================================
// 入口
// ===========================================================================

/// 对一个文件做 desugar reshape（in-place 改写 items）。
///
/// `allocated` 是 parse 阶段已分配的 NodeId 数量，合成节点从此之后编号。
pub fn reshape_file(file: &mut crate::ast::File, interner: &mut Interner, allocated: usize) {
    let mut cx = Cx {
        ids: NodeIdAllocator::new_starting_at(allocated),
        interner,
    };
    for item in &mut file.items {
        reshape_item(item, &mut cx);
    }
}

/// desugar 上下文：唯一的可变状态是 NodeIdAllocator（+ interner for 合成符号）。
struct Cx<'a> {
    ids: NodeIdAllocator,
    interner: &'a mut Interner,
}

impl<'a> Cx<'a> {
    /// 分配一个新 NodeId。
    fn nid(&mut self) -> NodeId {
        self.ids.alloc()
    }

    /// 分配一个 span（合成节点用原糖节点的 span，便于错误定位）。
    fn span(&self, s: Span) -> Span {
        s
    }

    /// intern 一个符号（合成标识符用）。
    fn sym(&mut self, text: &str) -> Symbol {
        self.interner.intern(text)
    }

    /// 构造一个合成 Ident（给定名字 + span）。
    fn ident(&mut self, text: &str, span: Span) -> Ident {
        Ident {
            symbol: self.sym(text),
            span,
        }
    }
}

// ===========================================================================
// Item / 语句 / 表达式 / 类型 递归骨架
// ===========================================================================

fn reshape_item(item: &mut Item, cx: &mut Cx) {
    match &mut item.kind {
        ItemKind::Fun(d) => reshape_fun_decl(d, cx),
        ItemKind::Val(d) => reshape_val_decl(d, cx),
        ItemKind::TypeAlias(d) => reshape_type_alias(d, cx),
        ItemKind::Type(d) => reshape_type_decl(d, cx),
        ItemKind::ExtensionProperty(_) | ItemKind::Object(_) => {
            // 这些项内的类型/表达式也需递归（暂未深入；按需补）。
        }
    }
}

fn reshape_fun_decl(d: &mut FunDecl, cx: &mut Cx) {
    // 参数类型注解
    for p in &mut d.params {
        if let Some(t) = p.ty.as_mut() {
            reshape_type(t, cx);
        }
    }
    // 返回类型
    if let Some(t) = d.return_ty.as_mut() {
        reshape_type(t, cx);
    }
    // 函数体
    if let Some(body) = d.body.as_mut() {
        match body {
            FunBody::Block(b) => reshape_block(b, cx),
            FunBody::Expr(e) => reshape_expr(e, cx),
        }
    }
}

fn reshape_val_decl(d: &mut ValDecl, cx: &mut Cx) {
    if let Some(t) = d.ty.as_mut() {
        reshape_type(t, cx);
    }
    if let Some(init) = d.init.as_mut() {
        reshape_expr(init, cx);
    }
}

fn reshape_type_alias(d: &mut TypeAliasDecl, cx: &mut Cx) {
    reshape_type(&mut d.ty, cx);
}

fn reshape_type_decl(d: &mut TypeDecl, cx: &mut Cx) {
    // primary ctor 参数类型
    if let Some(ctor) = d.primary_ctor.as_mut() {
        for p in &mut ctor.params {
            if let Some(t) = p.ty.as_mut() {
                reshape_type(t, cx);
            }
        }
    }
    // body 成员里的类型 + 初始化器
    if let Some(body) = d.body.as_mut() {
        for m in &mut body.members {
            use crate::ast::decl::TypeMemberKind;
            match &mut m.kind {
                TypeMemberKind::Property(pd) => {
                    if let Some(t) = pd.ty.as_mut() {
                        reshape_type(t, cx);
                    }
                    if let Some(init) = pd.init.as_mut() {
                        reshape_expr(init, cx);
                    }
                }
                TypeMemberKind::Fun(fd) => reshape_fun_decl(fd, cx),
                TypeMemberKind::SecondaryCtor(sc) => {
                    for p in &mut sc.params {
                        if let Some(t) = p.ty.as_mut() {
                            reshape_type(t, cx);
                        }
                    }
                    reshape_block(&mut sc.body, cx);
                }
                TypeMemberKind::InitBlock(ib) => reshape_block(&mut ib.body, cx),
                TypeMemberKind::EnumVariant(ev) => {
                    for f in &mut ev.fields {
                        reshape_type(&mut f.ty, cx);
                    }
                    if let Some(disc) = ev.discriminant.as_mut() {
                        reshape_expr(disc, cx);
                    }
                }
                TypeMemberKind::Object(_) | TypeMemberKind::Type(_) => {
                    // 嵌套 object/type：暂不深入（其体类型/初始化器罕见，按需补）。
                }
            }
        }
    }
}

fn reshape_block(block: &mut Block, cx: &mut Cx) {
    for stmt in &mut block.stmts {
        reshape_stmt(stmt, cx);
    }
}

fn reshape_stmt(stmt: &mut Stmt, cx: &mut Cx) {
    match &mut stmt.kind {
        StmtKind::Empty | StmtKind::Break | StmtKind::Continue => {}
        StmtKind::Expr(e) => reshape_expr(e, cx),
        StmtKind::Assign { target, value } => {
            reshape_expr(value, cx);
            // 项3：a[i, j] = v → a.set(i, j, v)（operator set）。
            // 其余赋值目标（Ident / Member）保留原样（Member 赋值是字段写，非方法调用）。
            let is_index = matches!(target.kind, AssignTargetKind::Index { .. });
            if is_index {
                let span = stmt.span;
                // 取出 target 的内容（替换为 Ident 占位以解除借用）。
                let placeholder_kind = AssignTargetKind::Ident(cx.ident("__placeholder", span));
                let tk = std::mem::replace(&mut target.kind, placeholder_kind);
                let (recv, idxs) = match tk {
                    AssignTargetKind::Index {
                        receiver,
                        indices,
                    } => (*receiver, indices),
                    _ => unreachable!(),
                };
                // value 已 reshape；取出。
                let val = std::mem::replace(
                    value,
                    Expr {
                        id: cx.nid(),
                        span,
                        kind: ExprKind::UnitLit,
                    },
                );
                let mut set_args: Vec<Expr> = idxs;
                set_args.push(val);
                let call = make_method_call(cx, recv, "set", set_args, span);
                stmt.kind = StmtKind::Expr(call);
            } else {
                reshape_assign_target(target, cx);
            }
        }
        StmtKind::LocalVal(v) => reshape_val_decl(v, cx),
        StmtKind::Return { value } => {
            if let Some(e) = value {
                reshape_expr(e, cx);
            }
        }
        StmtKind::While { cond, body } => {
            reshape_expr(cond, cx);
            reshape_block(body, cx);
        }
        StmtKind::For {
            binder: _,
            iter,
            body,
        } => {
            // 先递归子节点（for body 内的 for 也会被处理）。
            reshape_expr(iter, cx);
            reshape_block(body, cx);
            // for-loop 的 desugar 项7 实现（待加）。
        }
    }
}

fn reshape_assign_target(target: &mut AssignTarget, cx: &mut Cx) {
    match &mut target.kind {
        AssignTargetKind::Ident(_) => {}
        AssignTargetKind::Member { receiver, .. } => reshape_expr(receiver, cx),
        AssignTargetKind::Index {
            receiver,
            indices,
        } => {
            reshape_expr(receiver, cx);
            for i in indices {
                reshape_expr(i, cx);
            }
            // index-assign desugar（a[i]=v → a.set(i,v)）项3 待加。
        }
    }
}

fn reshape_expr(expr: &mut Expr, cx: &mut Cx) {
    match &mut expr.kind {
        ExprKind::Ident(_)
        | ExprKind::IntLit(_)
        | ExprKind::FloatLit(_)
        | ExprKind::CharLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::UnitLit
        | ExprKind::ClassLit { .. } => {}
        ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let crate::ast::StringPart::Expr(e) = p {
                    reshape_expr(e, cx);
                }
            }
            // f"..." desugar 项6 待加。
        }
        ExprKind::TupleLit(els) | ExprKind::ArrayLit(els) => {
            for e in els {
                reshape_expr(e, cx);
            }
        }
        ExprKind::StructLit { name: _, fields } => {
            for f in fields {
                reshape_expr(&mut f.value, cx);
            }
        }
        ExprKind::Block(b)
        | ExprKind::DoBlock(b)
        | ExprKind::UnsafeBlock(b)
        | ExprKind::SafeBlock(b) => reshape_block(b, cx),
        ExprKind::Lambda(l) => reshape_lambda(l, cx),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            reshape_expr(cond, cx);
            reshape_expr(then_branch, cx);
            if let Some(e) = else_branch {
                reshape_expr(e, cx);
            }
        }
        ExprKind::When { subject, arms } => {
            reshape_expr(subject, cx);
            for arm in arms {
                if let Some(g) = arm.guard.as_mut() {
                    reshape_expr(g, cx);
                }
                reshape_expr(&mut arm.body, cx);
            }
        }
        ExprKind::Handle { body, arms, finally } => {
            reshape_block(body, cx);
            for arm in arms {
                reshape_expr(&mut arm.body, cx);
            }
            if let Some(f) = finally {
                reshape_block(f, cx);
            }
        }
        ExprKind::MemberAccess { receiver, .. } => {
            reshape_expr(receiver, cx);
        }
        ExprKind::SafeMemberAccess { receiver, member } => {
            reshape_expr(receiver, cx);
            // 项5：a?.b → when(a){Some(v)->v.b; None->None}。
            let span = expr.span;
            let recv = std::mem::replace(
                receiver.as_mut(),
                Expr {
                    id: cx.nid(),
                    span,
                    kind: ExprKind::UnitLit,
                },
            );
            // 合成绑定名（唯一：带原 receiver 的 NodeId 后缀避免撞用户名）。
            let binder_name = format!("__safe_{}", recv.id.as_u32());
            let binder = cx.ident(&binder_name, span);
            let binder_for_pat = binder;
            // v.b：用合成绑定作 receiver。
            let v_expr = Expr {
                id: cx.nid(),
                span,
                kind: ExprKind::Ident(binder),
            };
            let access = Expr {
                id: cx.nid(),
                span,
                kind: ExprKind::MemberAccess {
                    receiver: Box::new(v_expr),
                    member: *member,
                },
            };
            // Some 分支体：把访问结果包回 Some(...)，使两分支同为 Option<T>。
            let some_body = make_variant_call(cx, "Some", vec![access], span);
            // None 字面量（variant 引用）。
            let none_expr = Expr {
                id: cx.nid(),
                span,
                kind: ExprKind::Ident(cx.ident("None", span)),
            };
            let some_pat = make_some_pattern(cx, binder_for_pat, span);
            let none_pat = make_none_pattern(cx, span);
            let some_arm = make_when_arm(cx, some_pat, some_body, span);
            let none_arm = make_when_arm(cx, none_pat, none_expr, span);
            let when_id = cx.nid();
            *expr = Expr {
                id: when_id,
                span,
                kind: ExprKind::When {
                    subject: Box::new(recv),
                    arms: vec![some_arm, none_arm],
                },
            };
        }
        ExprKind::SpliceField { receiver, field } => {
            reshape_expr(receiver, cx);
            reshape_expr(field, cx);
        }
        ExprKind::Index {
            receiver,
            indices,
        } => {
            reshape_expr(receiver, cx);
            for i in indices.iter_mut() {
                reshape_expr(i, cx);
            }
            // 项3：a[i, j] → a.get(i, j)。多下标一次性传入 get（operator get 解析在 typecheck）。
            let span = expr.span;
            let recv = std::mem::replace(
                receiver.as_mut(),
                Expr {
                    id: cx.nid(),
                    span,
                    kind: ExprKind::UnitLit,
                },
            );
            let idxs: Vec<Expr> = std::mem::take(indices);
            *expr = make_method_call(cx, recv, "get", idxs, span);
        }
        ExprKind::NotNullAssert { expr: inner } => {
            reshape_expr(inner, cx);
            // 项4：a!! → a.unwrap()（may panic，符合预期）。
            let span = expr.span;
            let operand = std::mem::replace(
                inner.as_mut(),
                Expr {
                    id: cx.nid(),
                    span,
                    kind: ExprKind::UnitLit,
                },
            );
            *expr = make_method_call(cx, operand, "unwrap", vec![], span);
        }
        ExprKind::TypeApply { callee, args } => {
            reshape_expr(callee, cx);
            for a in args {
                if let TypeArgKind::Type(t) = &mut a.kind {
                    reshape_type(t, cx);
                }
            }
        }
        ExprKind::Call { callee, args } => {
            reshape_expr(callee, cx);
            for a in args {
                reshape_expr(&mut a.value, cx);
            }
        }
        ExprKind::Unary { op, expr: inner } => {
            reshape_expr(inner, cx);
            // 项2：-x → x.unaryMinus()，~x → x.bitNot()。!x（Bool 取反）保留。
            if let Some(method) = unop_method(*op) {
                let span = expr.span;
                let operand = std::mem::replace(
                    inner.as_mut(),
                    Expr {
                        id: cx.nid(),
                        span,
                        kind: ExprKind::UnitLit,
                    },
                );
                *expr = make_method_call(cx, operand, method, vec![], span);
            }
        }
        ExprKind::Binary { lhs, rhs, op } => {
            reshape_expr(lhs, cx);
            reshape_expr(rhs, cx);
            // 项2：a + b → a.plus(b) 等。LogAnd/LogOr/Elvis/Range 保留（短路/特殊语义）。
            if let Some(method) = binop_method(*op) {
                let span = expr.span;
                let lhs_e = std::mem::replace(
                    lhs.as_mut(),
                    Expr {
                        id: cx.nid(),
                        span,
                        kind: ExprKind::UnitLit,
                    },
                );
                let rhs_e = std::mem::replace(
                    rhs.as_mut(),
                    Expr {
                        id: cx.nid(),
                        span,
                        kind: ExprKind::UnitLit,
                    },
                );
                *expr = make_method_call(cx, lhs_e, method, vec![rhs_e], span);
            }
        }
        ExprKind::InfixCall {
            receiver,
            arg,
            ..
        } => {
            reshape_expr(receiver, cx);
            reshape_expr(arg, cx);
            // infix operator desugar 待加（遇阻可暂缓）。
        }
        ExprKind::TypeCheck { expr: inner, ty, .. } | ExprKind::Cast { expr: inner, ty, .. } => {
            reshape_expr(inner, cx);
            reshape_type(ty, cx);
        }
        ExprKind::WithUpdate { base, updates } => {
            reshape_expr(base, cx);
            for u in updates {
                reshape_expr(&mut u.value, cx);
            }
            // with 是 value type 基础操作，不 desugar。
        }
        ExprKind::Annotated {
            expr: inner, ..
        } => reshape_expr(inner, cx),
    }
}

fn reshape_lambda(l: &mut LambdaExpr, cx: &mut Cx) {
    for p in &mut l.params {
        if let Some(t) = p.ty.as_mut() {
            reshape_type(t, cx);
        }
    }
    match &mut l.body {
        crate::ast::LambdaBody::Block(b) => reshape_block(b, cx),
        crate::ast::LambdaBody::Expr(e) => reshape_expr(e, cx),
    }
}

// ===========================================================================
// 类型 reshape（项1：T? → Option<T>）
// ===========================================================================

fn reshape_type(ty: &mut TypeRef, cx: &mut Cx) {
    match &mut ty.kind {
        // T? → Option<T>：把 Nullable(inner) 改写成 Path{Option<T>}。
        // 先递归 inner（inner 里的 T? 也要展开），再替换当前节点。
        TypeRefKind::Nullable(inner) => {
            reshape_type(inner, cx);
            let inner_taken = std::mem::replace(
                &mut inner.kind,
                TypeRefKind::Unit,
            );
            let option_path = make_option_path(cx, ty.span);
            let arg = TypeArg {
                id: cx.nid(),
                span: ty.span,
                kind: TypeArgKind::Type(TypeRef {
                    id: cx.nid(),
                    span: ty.span,
                    kind: inner_taken,
                }),
            };
            ty.kind = TypeRefKind::Path {
                path: option_path,
                args: vec![arg],
            };
        }
        TypeRefKind::Path { path: _, args } => {
            for a in args {
                if let TypeArgKind::Type(t) = &mut a.kind {
                    reshape_type(t, cx);
                }
            }
        }
        TypeRefKind::Tuple(els) => {
            for e in els {
                reshape_type(e, cx);
            }
        }
        TypeRefKind::Function {
            params,
            ret,
            effect: _,
        } => {
            for p in params {
                reshape_type(p, cx);
            }
            reshape_type(ret, cx);
        }
        TypeRefKind::ReceiverFunction {
            receiver,
            params,
            ret,
            effect: _,
        } => {
            reshape_type(receiver, cx);
            for p in params {
                reshape_type(p, cx);
            }
            reshape_type(ret, cx);
        }
        TypeRefKind::Unit => {}
    }
}

/// 构造 `Option` 类型路径（合成 segments：`Option` 单段，resolve 时按 import/package 解析）。
fn make_option_path(cx: &mut Cx, span: Span) -> TypePath {
    TypePath {
        segments: vec![cx.ident("Option", span)],
        span,
    }
}

// ===========================================================================
// operator → method name 映射（项2）
// ===========================================================================

/// BinaryOp → 方法名。`None` 表示该运算符**不** desugar（短路/特殊语义）。
///
/// - `LogAnd`/`LogOr`/`Elvis`：短路求值，不能展开成方法调用。
/// - `Range`（`..`）：保留为运算符节点（产物形态另定）。
fn binop_method(op: crate::ast::expr::BinaryOp) -> Option<&'static str> {
    use crate::ast::expr::BinaryOp as B;
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
        B::Range | B::LogAnd | B::LogOr | B::Elvis => return None,
    })
}

/// UnaryOp → 方法名。`None` 表示不 desugar。
///
/// - `Not`（`!x`，Bool 取反）：保留（走专门路径）。
fn unop_method(op: crate::ast::expr::UnaryOp) -> Option<&'static str> {
    use crate::ast::expr::UnaryOp as U;
    Some(match op {
        U::Neg => "unaryMinus",
        U::BitNot => "bitNot",
        U::Not => return None,
    })
}

// ===========================================================================
// 构造辅助（合成表达式节点）
// ===========================================================================

fn make_ident_expr(cx: &mut Cx, text: &str, span: Span) -> Expr {
    Expr {
        id: cx.nid(),
        span,
        kind: ExprKind::Ident(cx.ident(text, span)),
    }
}

/// 构造 `receiver.method(args)` 调用表达式。
fn make_method_call(
    cx: &mut Cx,
    receiver: Expr,
    method: &str,
    args: Vec<Expr>,
    span: Span,
) -> Expr {
    let member = Expr {
        id: cx.nid(),
        span,
        kind: ExprKind::MemberAccess {
            receiver: Box::new(receiver),
            member: MemberName::Named(cx.ident(method, span)),
        },
    };
    let call_args: Vec<CallArg> = args
        .into_iter()
        .map(|a| CallArg {
            id: cx.nid(),
            span: a.span,
            name: None,
            is_spread: false,
            value: a,
        })
        .collect();
    Expr {
        id: cx.nid(),
        span,
        kind: ExprKind::Call {
            callee: Box::new(member),
            args: call_args,
        },
    }
}

/// 构造一个 `Some(x)` variant 模式。
fn make_some_pattern(cx: &mut Cx, binder: Ident, span: Span) -> Pattern {
    Pattern {
        id: cx.nid(),
        span,
        kind: PatternKind::Variant {
            path: TypePath {
                segments: vec![cx.ident("Some", span)],
                span,
            },
            args: Some(vec![Pattern {
                id: cx.nid(),
                span: binder.span,
                kind: PatternKind::Bind(binder),
            }]),
        },
    }
}

/// 构造一个 `None` variant 模式（无参数）。
fn make_none_pattern(cx: &mut Cx, span: Span) -> Pattern {
    Pattern {
        id: cx.nid(),
        span,
        kind: PatternKind::Variant {
            path: TypePath {
                segments: vec![cx.ident("None", span)],
                span,
            },
            args: None,
        },
    }
}

fn make_when_arm(cx: &mut Cx, pat: Pattern, body: Expr, span: Span) -> WhenArm {
    WhenArm {
        id: cx.nid(),
        span,
        pat,
        guard: None,
        body,
    }
}

/// 构造 variant 调用 `Name(args...)`（如 `Some(x)`）。
fn make_variant_call(cx: &mut Cx, name: &str, args: Vec<Expr>, span: Span) -> Expr {
    let callee = Expr {
        id: cx.nid(),
        span,
        kind: ExprKind::Ident(cx.ident(name, span)),
    };
    let call_args: Vec<CallArg> = args
        .into_iter()
        .map(|a| CallArg {
            id: cx.nid(),
            span: a.span,
            name: None,
            is_spread: false,
            value: a,
        })
        .collect();
    Expr {
        id: cx.nid(),
        span,
        kind: ExprKind::Call {
            callee: Box::new(callee),
            args: call_args,
        },
    }
}

#[allow(dead_code)]
fn _unused(_nid: NodeId) {}
