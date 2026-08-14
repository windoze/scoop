//! HIR body 树（PLAN.md M2-2）：desugar 后的**封闭节点词汇表**（C9-1）。
//!
//! 词汇表即契约：本枚举**不含**任何糖变体——`Binary`/`Unary`/`InfixCall` 在构造时
//! 展开为 `Call`（typecheck 已给出决议），`NotNullAssert`（`!!`）展开为
//! `Option.unwrap` 调用。下游对 [`TreeExprKind`] 的穷尽 match 写不出「处理糖」的
//! 分支；尚未能正确构造的构造记入 `gaps`（transitional——M2 完成前允许非空，
//! MIR 翻转时 gaps 必须为空，届时词汇表与 builder 同步收口）。
//!
//! 决议内联（C9-2）：每个节点直接携带推断类型（`ty`，裸非 Option——合法性由
//! completeness gate 上游保证）与决议句柄；[`SemanticFacts`] 侧表在此折叠为节点
//! 字段。局部绑定是 body 内 [`LocalId`]（C3 第三层，非 def、不外泄）。
//!
//! 物理表示：每 body 一组 arena（exprs/stmts/blocks/locals），id 为下标（C7——
//! 构造顺序即 AST 遍历顺序，确定）。

use scoop2_base::{NodeId, Span, Symbol};

use crate::resolve::output::NodeIdTable;
use crate::ty::TypeId;

use super::facts::SemanticFacts;

// ---------------------------------------------------------------------------
// id 与容器
// ---------------------------------------------------------------------------

macro_rules! tree_id {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
        pub struct $name(pub u32);

        impl $name {
            #[inline]
            fn idx(self) -> usize {
                self.0 as usize
            }
        }
    };
}

tree_id!(ExprId);
tree_id!(StmtId);
tree_id!(BlockId);
tree_id!(LocalId);

/// 一个函数体的树：arena 集合 + 根块。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreeBody {
    pub exprs: Vec<TreeExpr>,
    pub stmts: Vec<TreeStmt>,
    pub blocks: Vec<TreeBlock>,
    pub locals: Vec<TreeLocal>,
    /// 根块（函数体 / init 块入口）。
    pub root: Option<BlockId>,
}

/// 一个函数的树产物（挂在 [`super::TypedFile`] 上）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FnTree {
    /// 函数 FQN 文本（跨阶段稳定身份过渡用；M2 element 体系后换 def id）。
    pub fqn: String,
    /// 参数绑定（与声明序平行）。
    pub params: Vec<LocalId>,
    pub body: TreeBody,
    /// 构造缺口（transitional）：未能构造的构造点（span + 描述）。MIR 翻转前
    /// 必须清空；非空表示该函数的树不可消费。
    pub gaps: Vec<(Span, String)>,
}

/// 局部绑定（参数 / `val`/`var` / 模式子绑定）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeLocal {
    pub name: Symbol,
    pub ty: TypeId,
    pub mutable: bool,
    pub span: Span,
}

/// 块：语句序列 + 尾表达式（块是表达式，带尾值）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeBlock {
    pub span: Span,
    pub stmts: Vec<StmtId>,
    pub tail: Option<ExprId>,
}

// ---------------------------------------------------------------------------
// 表达式（desugar 后封闭词汇表）
// ---------------------------------------------------------------------------

/// 表达式节点：kind + 推断类型 + span（类型是裸的，非 Option——C9-2）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeExpr {
    pub kind: TreeExprKind,
    pub ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeExprKind {
    /// 字面量。
    Lit(Lit),
    /// 局部绑定引用。
    LocalRef(LocalId),
    /// 顶层 `val`/`var` 引用。
    TopLevelValRef { fqn: Symbol },
    /// 调用（普通 / 方法 / 构造 / variant / 函数值 / perform——`Binary`/`Unary`/
    /// `InfixCall`/`!!`/下标全部收敛于此）。
    Call {
        callee: TreeCallee,
        args: Vec<ExprId>,
    },
    /// 成员读取（字段 / 元组下标）。
    Member { recv: ExprId, member: TreeMember },
    /// 块表达式（含 `do` 块）。
    Block(BlockId),
    /// `if`（分支为块表达式）。
    If {
        cond: ExprId,
        then: ExprId,
        else_: Option<ExprId>,
    },
    /// `while`（Unit 值）。
    While { cond: ExprId, body: BlockId },
    /// 元组字面量。
    Tuple(Vec<ExprId>),
    /// 数组字面量。
    ArrayLit(Vec<ExprId>),
}

/// 字面量值。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Lit {
    Unit,
    Bool(bool),
    Int(u128),
    Float(f64),
    Char(char),
    Str(String),
}

/// 已解析的被调用方（从 [`super::ResolvedCall`] 折叠）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeCallee {
    /// 顶层函数直接调用。
    TopLevel { fqn: Symbol },
    /// 方法调用（含运算符糖展开；receiver 是参数 0 之前的接收者表达式）。
    Method {
        recv: ExprId,
        owner_fqn: Symbol,
        method: Symbol,
    },
    /// 构造器调用。
    Ctor { type_fqn: Symbol },
    /// enum variant 构造。
    Variant { enum_fqn: Symbol, variant: Symbol },
    /// 局部函数值调用。
    LocalValue { local: LocalId },
    /// 任意函数值表达式调用（`f()(x)` / `fns[0](1)` / lambda 调用）。
    FunValue { callee: ExprId },
    /// effect 操作（perform 站点）。
    EffectOp { effect: Symbol, op: Symbol },
}

/// 已解析的成员读取（从 [`super::ResolvedMember`] 折叠）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeMember {
    Field { owner_fqn: Symbol, name: Symbol },
    TupleIndex { index: u64 },
}

// ---------------------------------------------------------------------------
// 语句
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeStmt {
    /// 表达式语句。
    Expr(ExprId),
    /// 局部 `val`/`var`（`Name` 绑定；模式绑定见 `gaps`）。
    LocalVal { local: LocalId, init: ExprId },
    /// 赋值（LHS 已解析为 place）。
    Assign { place: TreePlace, value: ExprId },
    /// `return expr?`。
    Return(Option<ExprId>),
    /// `break`（绑定最近 enclosing while）。
    Break,
    /// `continue`。
    Continue,
}

/// 已解析的赋值目标（从 [`super::ResolvedPlace`] 折叠）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreePlace {
    Local(LocalId),
    TopLevelVar {
        fqn: Symbol,
    },
    /// `recv.name = v`。
    MemberField {
        recv: ExprId,
        owner_fqn: Symbol,
        name: Symbol,
    },
}

// ---------------------------------------------------------------------------
// builder：AST + facts → 树
// ---------------------------------------------------------------------------

/// 从 typecheck 产物构造一个函数体的树。
///
/// - `body`：函数体 AST（typecheck 后的形态——for-loop 等已在 typecheck desugar）。
/// - `params`：参数 (名, 类型) 序列（声明序）。
/// - `unit_ty`：Unit 的 `TypeId`（while 等无值构造的类型）。
/// - `expr_types` / `facts`：该文件的 typecheck 侧表。
///
/// 未覆盖构造记入 `gaps`（不静默、不猜测）。
pub fn build_fn_tree(
    fqn: String,
    body: &crate::syntax::ast::FunBody,
    params: &[(Symbol, TypeId)],
    unit_ty: TypeId,
    expr_types: &NodeIdTable<TypeId>,
    facts: &SemanticFacts,
) -> FnTree {
    let mut b = TreeBuilder {
        expr_types,
        facts,
        unit_ty,
        out: TreeBody::default(),
        scopes: vec![std::collections::HashMap::new()],
        gaps: Vec::new(),
    };
    let param_locals: Vec<LocalId> = params
        .iter()
        .map(|&(name, ty)| {
            b.push_local(
                name,
                ty,
                false,
                // 参数 span：局部无精确 span 时用 body 起点近似（诊断用途）。
                Span::new(0, 0),
            )
        })
        .collect();
    let root = match body {
        crate::syntax::ast::FunBody::Block(block) => b.build_block(block),
        crate::syntax::ast::FunBody::Expr(e) => {
            // `= expr` 短体：等价于 [return expr]。
            let block_id = b.fresh_block(Span::new(0, 0));
            if let Some(expr) = b.build_expr(e) {
                let stmt = b.push_stmt(TreeStmt::Return(Some(expr)));
                b.out.blocks[block_id.idx()].stmts.push(stmt);
            }
            block_id
        }
    };
    b.out.root = Some(root);
    FnTree {
        fqn,
        params: param_locals,
        body: b.out,
        gaps: b.gaps,
    }
}

struct TreeBuilder<'a> {
    expr_types: &'a NodeIdTable<TypeId>,
    facts: &'a SemanticFacts,
    unit_ty: TypeId,
    out: TreeBody,
    /// 词法作用域栈（块进栈/出栈）。
    scopes: Vec<std::collections::HashMap<Symbol, LocalId>>,
    gaps: Vec<(Span, String)>,
}

impl<'a> TreeBuilder<'a> {
    fn gap(&mut self, span: Span, what: &str) {
        self.gaps.push((span, what.to_string()));
    }

    fn push_local(&mut self, name: Symbol, ty: TypeId, mutable: bool, span: Span) -> LocalId {
        let id = LocalId(self.out.locals.len() as u32);
        self.out.locals.push(TreeLocal {
            name,
            ty,
            mutable,
            span,
        });
        self.scopes
            .last_mut()
            .expect("作用域栈非空")
            .insert(name, id);
        id
    }

    fn lookup_local(&self, name: Symbol) -> Option<LocalId> {
        self.scopes.iter().rev().find_map(|s| s.get(&name).copied())
    }

    fn fresh_block(&mut self, span: Span) -> BlockId {
        let id = BlockId(self.out.blocks.len() as u32);
        self.out.blocks.push(TreeBlock {
            span,
            stmts: Vec::new(),
            tail: None,
        });
        id
    }

    fn push_stmt(&mut self, stmt: TreeStmt) -> StmtId {
        let id = StmtId(self.out.stmts.len() as u32);
        self.out.stmts.push(stmt);
        id
    }

    fn push_expr(&mut self, kind: TreeExprKind, ty: TypeId, span: Span) -> ExprId {
        let id = ExprId(self.out.exprs.len() as u32);
        self.out.exprs.push(TreeExpr { kind, ty, span });
        id
    }

    /// 节点类型（completeness 上游保证存在；缺失记 gap 并用 Unit 兜底构造失败路径）。
    fn ty_of(&mut self, id: NodeId, span: Span) -> Option<TypeId> {
        match self.expr_types.get(id) {
            Some(&ty) => Some(ty),
            None => {
                self.gap(span, "expr_types 缺失（completeness 泄漏）");
                None
            }
        }
    }

    fn build_block(&mut self, block: &crate::syntax::ast::Block) -> BlockId {
        self.scopes.push(std::collections::HashMap::new());
        let block_id = self.fresh_block(block.span);
        for stmt in &block.stmts {
            let stmt_id = self.build_stmt(stmt);
            if let Some(sid) = stmt_id {
                self.out.blocks[block_id.idx()].stmts.push(sid);
            }
        }
        // 尾值：块无尾语句时 stmts 末尾是表达式语句——AST 把尾表达式解析为
        // StmtKind::Expr；这里把「最后一个表达式语句」提升为 tail。
        let stmts_len = self.out.blocks[block_id.idx()].stmts.len();
        if stmts_len > 0 {
            let last = *self.out.blocks[block_id.idx()].stmts.last().expect("非空");
            if let TreeStmt::Expr(e) = self.out.stmts[last.idx()] {
                // 仅当源块没有尾分号时才是尾值；保守起见一律提升（语义等价：
                // 表达式语句的值被丢弃 vs 尾值——函数体需要尾值，普通块提升
                // 不改变 MIR lower 结果）。
                self.out.blocks[block_id.idx()].tail = Some(e);
                self.out.blocks[block_id.idx()]
                    .stmts
                    .truncate(stmts_len - 1);
            }
        }
        self.scopes.pop();
        block_id
    }

    fn build_stmt(&mut self, stmt: &crate::syntax::ast::Stmt) -> Option<StmtId> {
        use crate::syntax::ast::StmtKind;
        match &stmt.kind {
            StmtKind::Empty => None,
            StmtKind::Expr(e) => self
                .build_expr(e)
                .map(|id| self.push_stmt(TreeStmt::Expr(id))),
            StmtKind::Return { value } => {
                let v = value.as_ref().and_then(|e| self.build_expr(e));
                Some(self.push_stmt(TreeStmt::Return(v)))
            }
            StmtKind::Break => Some(self.push_stmt(TreeStmt::Break)),
            StmtKind::Continue => Some(self.push_stmt(TreeStmt::Continue)),
            StmtKind::LocalVal(d) => {
                let init_ast = match &d.init {
                    Some(e) => e,
                    None => {
                        self.gap(stmt.span, "局部声明缺初始化表达式");
                        return None;
                    }
                };
                let init = match self.build_expr(init_ast) {
                    Some(id) => id,
                    None => return None,
                };
                match &d.binding {
                    crate::syntax::ast::ValBinding::Name(name) => {
                        let Some(&ty) = self.expr_types.get(init_ast.id) else {
                            self.gap(stmt.span, "局部声明类型缺失（completeness 泄漏）");
                            return None;
                        };
                        let mutable = d.kind == crate::syntax::ast::ValKind::Var;
                        let local = self.push_local(name.symbol, ty, mutable, name.span);
                        Some(self.push_stmt(TreeStmt::LocalVal { local, init }))
                    }
                    crate::syntax::ast::ValBinding::Pattern(_) => {
                        self.gap(stmt.span, "模式解构绑定（M2 后续覆盖）");
                        None
                    }
                }
            }
            StmtKind::Assign { target, value } => {
                let Some(value) = self.build_expr(value) else {
                    return None;
                };
                let place = self.build_place(target);
                place.map(|p| self.push_stmt(TreeStmt::Assign { place: p, value }))
            }
            StmtKind::While { cond, body } => {
                let Some(cond) = self.build_expr(cond) else {
                    return None;
                };
                let body_block = self.build_block(body);
                let while_expr = self.push_expr(
                    TreeExprKind::While {
                        cond,
                        body: body_block,
                    },
                    self.unit_ty,
                    stmt.span,
                );
                Some(self.push_stmt(TreeStmt::Expr(while_expr)))
            }
            StmtKind::For { .. } => {
                // for 已在 typecheck desugar；到达这里说明 desugar 未运行（C9：
                // 不写防御分支，gap 显式暴露）。
                self.gap(stmt.span, "for-loop（应已在 typecheck desugar）");
                None
            }
        }
    }

    fn build_place(&mut self, target: &crate::syntax::ast::AssignTarget) -> Option<TreePlace> {
        use crate::syntax::ast::AssignTargetKind;
        let Some(place) = self.facts.assign_places.get(target.id).cloned() else {
            self.gap(target.span, "赋值目标 place 决议缺失（completeness 泄漏）");
            return None;
        };
        match place {
            super::ResolvedPlace::Local { name, .. } => match self.lookup_local(name) {
                Some(local) => Some(TreePlace::Local(local)),
                None => self.gap_ret(target.span, "赋值局部超出词法作用域"),
            },
            super::ResolvedPlace::TopLevelVar { fqn, .. } => Some(TreePlace::TopLevelVar { fqn }),
            super::ResolvedPlace::MemberField {
                owner_fqn,
                member_name,
                ..
            } => {
                let recv = match &target.kind {
                    AssignTargetKind::Member { receiver, .. } => self.build_expr(receiver)?,
                    _ => return self.gap_ret(target.span, "成员赋值目标缺接收者"),
                };
                Some(TreePlace::MemberField {
                    recv,
                    owner_fqn,
                    name: member_name,
                })
            }
            super::ResolvedPlace::Index { .. } => {
                self.gap_ret(target.span, "下标赋值（operator set 展开，M2 后续覆盖）")
            }
        }
    }

    fn build_expr(&mut self, expr: &crate::syntax::ast::Expr) -> Option<ExprId> {
        use crate::syntax::ast::ExprKind;
        let span = expr.span;
        match &expr.kind {
            ExprKind::UnitLit => {
                let ty = self.ty_of(expr.id, span)?;
                Some(self.push_expr(TreeExprKind::Lit(Lit::Unit), ty, span))
            }
            ExprKind::IntLit(l) => {
                let ty = self.ty_of(expr.id, span)?;
                Some(self.push_expr(TreeExprKind::Lit(Lit::Int(l.value)), ty, span))
            }
            ExprKind::FloatLit(l) => {
                let ty = self.ty_of(expr.id, span)?;
                Some(self.push_expr(TreeExprKind::Lit(Lit::Float(l.value)), ty, span))
            }
            ExprKind::CharLit(l) => {
                let ty = self.ty_of(expr.id, span)?;
                Some(self.push_expr(TreeExprKind::Lit(Lit::Char(l.value)), ty, span))
            }
            ExprKind::StringLit(l) => {
                let ty = self.ty_of(expr.id, span)?;
                Some(self.push_expr(TreeExprKind::Lit(Lit::Str(l.value.clone())), ty, span))
            }
            ExprKind::Ident(id) => self.build_ident(expr, id),
            ExprKind::Binary { lhs, rhs, .. }
            | ExprKind::InfixCall {
                receiver: lhs,
                arg: rhs,
                ..
            } => {
                // 运算符糖 → 方法调用（决议已由 typecheck 写入 call_resolutions）。
                self.build_desugared_call(expr, &[lhs, rhs])
            }
            ExprKind::Unary { expr: inner, .. } => self.build_desugared_call(expr, &[inner]),
            ExprKind::NotNullAssert { expr: inner } => {
                // `!!` → Option.unwrap 调用（决议已记录）。
                self.build_desugared_call(expr, &[inner])
            }
            ExprKind::Index { receiver, indices } => {
                let mut args: Vec<&crate::syntax::ast::Expr> = vec![receiver];
                args.extend(indices.iter());
                self.build_desugared_call(expr, &args)
            }
            ExprKind::Call { callee, args } => self.build_call_expr(expr, callee, args),
            ExprKind::MemberAccess { receiver, .. } => self.build_member_expr(expr, receiver),
            ExprKind::Block(b) | ExprKind::DoBlock(b) => {
                let ty = self.ty_of(expr.id, span)?;
                let block = self.build_block(b);
                Some(self.push_expr(TreeExprKind::Block(block), ty, span))
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let ty = self.ty_of(expr.id, span)?;
                let cond = self.build_expr(cond)?;
                let then = self.build_expr(then_branch)?;
                let else_ = else_branch.as_ref().and_then(|e| self.build_expr(e));
                Some(self.push_expr(TreeExprKind::If { cond, then, else_ }, ty, span))
            }
            ExprKind::TupleLit(els) => {
                let ty = self.ty_of(expr.id, span)?;
                let items: Vec<ExprId> = els.iter().filter_map(|e| self.build_expr(e)).collect();
                if items.len() != els.len() {
                    return None;
                }
                Some(self.push_expr(TreeExprKind::Tuple(items), ty, span))
            }
            ExprKind::ArrayLit(els) => {
                let ty = self.ty_of(expr.id, span)?;
                let items: Vec<ExprId> = els.iter().filter_map(|e| self.build_expr(e)).collect();
                if items.len() != els.len() {
                    return None;
                }
                Some(self.push_expr(TreeExprKind::ArrayLit(items), ty, span))
            }
            // 注解表达式：语义在 typecheck 消费（@Suppress 等），树取内层表达式。
            ExprKind::Annotated { expr: inner, .. } => self.build_expr(inner),
            // ---- 尚未覆盖（记 gap，不静默）----
            ExprKind::When { .. } => self.gap_ret(span, "when（PatternMatch 词汇，M2 后续）"),
            ExprKind::Lambda(_) => self.gap_ret(span, "lambda/closure（M2 后续）"),
            ExprKind::Handle { .. } => self.gap_ret(span, "handle（M2 后续）"),
            ExprKind::Cast { .. } => self.gap_ret(span, "as 转换（M2 后续）"),
            ExprKind::TypeCheck { .. } => self.gap_ret(span, "is 类型判断（M2 后续）"),
            ExprKind::TypeApply { .. } => self.gap_ret(span, "显式类型应用（M2 后续）"),
            ExprKind::StructLit { .. } => self.gap_ret(span, "struct 字面量（M2 后续）"),
            ExprKind::WithUpdate { .. } => self.gap_ret(span, "with 更新（M2 后续）"),
            ExprKind::InterpolatedString { .. } => {
                self.gap_ret(span, "f-string（desugar 至调用链，M2 后续）")
            }
            ExprKind::SafeMemberAccess { .. } => {
                self.gap_ret(span, "?. 安全访问（短路展开，M2 后续）")
            }
            ExprKind::UnsafeBlock(_) | ExprKind::SafeBlock(_) => {
                self.gap_ret(span, "@Unsafe/@Safe 块（M2 后续）")
            }
            ExprKind::ClassLit { .. } => self.gap_ret(span, "T::class 反射字面量（M2 后续）"),
            ExprKind::SpliceField { .. } => self.gap_ret(span, "splice field（特性已移除）"),
        }
    }

    fn gap_ret<T>(&mut self, span: Span, what: &str) -> Option<T> {
        self.gap(span, what);
        None
    }

    fn build_ident(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        id: &crate::syntax::ast::Ident,
    ) -> Option<ExprId> {
        let span = expr.span;
        let ty = self.ty_of(expr.id, span)?;
        match self.facts.value_refs.get(expr.id) {
            Some(super::ResolvedValue::Local { .. }) => match self.lookup_local(id.symbol) {
                Some(local) => Some(self.push_expr(TreeExprKind::LocalRef(local), ty, span)),
                None => self.gap_ret(span, "局部引用超出词法作用域"),
            },
            Some(super::ResolvedValue::TopLevelValue { fqn }) => {
                Some(self.push_expr(TreeExprKind::TopLevelValRef { fqn: *fqn }, ty, span))
            }
            Some(super::ResolvedValue::TopLevelFun { .. }) => {
                self.gap_ret(span, "函数名做值（FunVal 变体，M2 后续）")
            }
            None => self.gap_ret(span, "value_refs 缺失（completeness 泄漏）"),
        }
    }

    /// 糖表达式（Binary/Unary/InfixCall/!!/Index）→ Call：决议在 call_resolutions。
    fn build_desugared_call(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        arg_exprs: &[&crate::syntax::ast::Expr],
    ) -> Option<ExprId> {
        let span = expr.span;
        let resolution = match self.facts.call_resolutions.get(expr.id) {
            Some(r) => r.clone(),
            None => return self.gap_ret(span, "糖调用决议缺失（completeness 泄漏）"),
        };
        let ty = self.ty_of(expr.id, span)?;
        let args: Vec<ExprId> = arg_exprs
            .iter()
            .filter_map(|e| self.build_expr(e))
            .collect();
        if args.len() != arg_exprs.len() {
            return None;
        }
        // 糖调用的 callee：Method 的接收者就是首个实参表达式（`a + b` → `a.plus(b)`）。
        let callee = match &resolution {
            super::ResolvedCall::Method {
                owner_fqn,
                method_name,
                ..
            } if !args.is_empty() => TreeCallee::Method {
                recv: args[0],
                owner_fqn: *owner_fqn,
                method: *method_name,
            },
            super::ResolvedCall::TopLevelFun { fqn, .. } => TreeCallee::TopLevel { fqn: *fqn },
            super::ResolvedCall::Constructor { type_fqn, .. } => TreeCallee::Ctor {
                type_fqn: *type_fqn,
            },
            super::ResolvedCall::EnumVariant {
                enum_fqn,
                variant_name,
                ..
            } => TreeCallee::Variant {
                enum_fqn: *enum_fqn,
                variant: *variant_name,
            },
            super::ResolvedCall::EffectOp {
                effect_name,
                op_name,
                ..
            } => TreeCallee::EffectOp {
                effect: *effect_name,
                op: *op_name,
            },
            _ => return self.gap_ret(span, "糖调用出现非预期决议形态"),
        };
        Some(self.push_expr(TreeExprKind::Call { callee, args }, ty, span))
    }

    fn build_call_expr(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        callee: &crate::syntax::ast::Expr,
        args: &[crate::syntax::ast::CallArg],
    ) -> Option<ExprId> {
        use crate::syntax::ast::ExprKind;
        let span = expr.span;
        let resolution = match self.facts.call_resolutions.get(expr.id) {
            Some(r) => r.clone(),
            None => return self.gap_ret(span, "call 决议缺失（completeness 泄漏）"),
        };
        let ty = self.ty_of(expr.id, span)?;
        let callee_id = self.callee_from_resolution(&resolution, callee, span)?;
        let mut arg_ids: Vec<ExprId> = Vec::with_capacity(args.len());
        for a in args {
            let Some(id) = self.build_expr(&a.value) else {
                return None;
            };
            arg_ids.push(id);
        }
        Some(self.push_expr(
            TreeExprKind::Call {
                callee: callee_id,
                args: arg_ids,
            },
            ty,
            span,
        ))
    }

    fn callee_from_resolution(
        &mut self,
        resolution: &super::ResolvedCall,
        callee: &crate::syntax::ast::Expr,
        span: Span,
    ) -> Option<TreeCallee> {
        use crate::syntax::ast::ExprKind;
        match resolution {
            super::ResolvedCall::TopLevelFun { fqn, .. } => {
                Some(TreeCallee::TopLevel { fqn: *fqn })
            }
            super::ResolvedCall::Method {
                owner_fqn,
                method_name,
                ..
            } => {
                // 接收者 = callee（MemberAccess）的 receiver 子表达式。
                let recv = match &callee.kind {
                    ExprKind::MemberAccess { receiver, .. } => self.build_expr(receiver)?,
                    _ => return self.gap_ret(span, "方法调用的 callee 不是成员访问"),
                };
                Some(TreeCallee::Method {
                    recv,
                    owner_fqn: *owner_fqn,
                    method: *method_name,
                })
            }
            super::ResolvedCall::Constructor { type_fqn, .. } => Some(TreeCallee::Ctor {
                type_fqn: *type_fqn,
            }),
            super::ResolvedCall::EnumVariant {
                enum_fqn,
                variant_name,
                ..
            } => Some(TreeCallee::Variant {
                enum_fqn: *enum_fqn,
                variant: *variant_name,
            }),
            super::ResolvedCall::LocalValue { local_name, .. } => {
                match self.lookup_local(*local_name) {
                    Some(local) => Some(TreeCallee::LocalValue { local }),
                    None => self.gap_ret(span, "LocalValue 调用目标超出词法作用域"),
                }
            }
            super::ResolvedCall::FunValue { .. } => {
                // callee 是任意函数类型表达式（`f()(x)` / `fns[0](1)` / lambda 调用）。
                let callee_expr = self.build_expr(callee)?;
                Some(TreeCallee::FunValue {
                    callee: callee_expr,
                })
            }
            super::ResolvedCall::EffectOp {
                effect_name,
                op_name,
                ..
            } => Some(TreeCallee::EffectOp {
                effect: *effect_name,
                op: *op_name,
            }),
        }
    }

    fn build_member_expr(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        receiver: &crate::syntax::ast::Expr,
    ) -> Option<ExprId> {
        let span = expr.span;
        let ty = self.ty_of(expr.id, span)?;
        let recv = self.build_expr(receiver)?;
        match self.facts.member_refs.get(expr.id) {
            Some(super::ResolvedMember::Field {
                owner_fqn,
                member_name,
                ..
            }) => Some(self.push_expr(
                TreeExprKind::Member {
                    recv,
                    member: TreeMember::Field {
                        owner_fqn: *owner_fqn,
                        name: *member_name,
                    },
                },
                ty,
                span,
            )),
            Some(super::ResolvedMember::TupleIndex { index, .. }) => Some(self.push_expr(
                TreeExprKind::Member {
                    recv,
                    member: TreeMember::TupleIndex {
                        index: *index as u64,
                    },
                },
                ty,
                span,
            )),
            Some(super::ResolvedMember::Method { .. }) => {
                self.gap_ret(span, "方法引用做值（FunVal 变体，M2 后续）")
            }
            None => self.gap_ret(span, "member 决议缺失（completeness 泄漏）"),
        }
    }
}
