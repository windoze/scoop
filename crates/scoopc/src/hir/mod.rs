//! HIR（High-level IR）。
//!
//! HIR 的定位：在 parser 产出的 AST 之上，引入一棵**已解析（name resolved）且已类型化（typed）**
//! 的中间表示，用于后续阶段：
//! - MIR lowering（显式控制流/临时变量）
//! - 单态化（monomorphization）
//! - LLVM codegen
//!
//! 当前阶段（TODO T0701）先落地“数据结构骨架 + 最小可用的 lowering”，用于：
//! - `scoop dump-hir <file>` 输出 Debug 视图，便于后续迭代与 fixtures 回归
//!
//! 注意：本模块尚未接入完整 typecheck/infer，因此未覆盖的表达式/语句会以 `Any` 作为类型占位，
//! 并用 `Todo(...)` 节点保留结构位置，避免 `panic!()` 阻断调试。

mod lower;

use std::fmt;

use crate::span::Span;
use crate::ty::TypeId;

pub use lower::{HirLowerError, LoweredHir, lower_for_dump};

/// HIR 中引用一个“已解析的符号”的稳定标识。
///
/// 说明：
/// - 当前阶段它主要用于把 AST 中的 ident 引用（`x`/`foo`）绑定到某个“解析结果”（local/top-level）；
/// - 后续若引入真正的全局 symbol table / 增量编译缓存，可把该 ID 扩展为跨 session 稳定的形式。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S{}", self.0)
    }
}

/// 一个 closure（lambda）在 HIR 中的稳定标识。
///
/// 说明：
/// - 该 ID 仅在“单文件 lowering”的 HIR 中稳定（用于 dump/fixtures 的可回归输出）；
/// - 后续若引入跨文件/跨 session 的 closure 符号表，可替换为更稳定的形式。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClosureId(u32);

impl ClosureId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for ClosureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "C{}", self.0)
    }
}

/// 一个源文件 lowering 后的 HIR。
#[derive(Debug, Clone)]
pub struct File {
    pub items: Vec<Item>,
}

/// 顶层条目（top-level items）。
#[derive(Debug, Clone)]
pub enum Item {
    Fun(FunDecl),
    Val(ValDecl),
    /// 未纳入当前阶段 HIR 的条目占位（例如 type/object/typealias 等）。
    Todo {
        span: Span,
        kind: &'static str,
    },
}

/// 函数声明（顶层或方法；当前阶段只做顶层 fun 的最小承载）。
#[derive(Debug, Clone)]
pub struct FunDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    /// 函数本身的类型（函数类型）。
    pub ty: TypeId,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub body: Option<Block>,
}

/// 函数参数（HIR 视图）。
#[derive(Debug, Clone)]
pub struct Param {
    pub span: Span,
    pub id: SymbolId,
    pub name: String,
    pub ty: TypeId,
}

/// `val`/`var` 声明（顶层或局部）。
#[derive(Debug, Clone)]
pub struct ValDecl {
    pub span: Span,
    pub id: Option<SymbolId>,
    pub name: Option<String>,
    pub mutable: bool,
    pub ty: TypeId,
    pub init: Option<Expr>,
}

/// 表达式块（block expression）。
#[derive(Debug, Clone)]
pub struct Block {
    pub span: Span,
    pub ty: TypeId,
    pub stmts: Vec<Stmt>,
}

/// 语句（statement）。
#[derive(Debug, Clone)]
pub struct Stmt {
    pub span: Span,
    pub ty: TypeId,
    pub kind: StmtKind,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Empty,
    Expr(Expr),
    Val(ValDecl),
    /// 赋值语句：`lhs = rhs`。
    ///
    /// 说明：虽然 parser 在 AST 中以 `ExprKind::Assign` 承载该语法，但在 HIR 中我们把它视为语句，
    /// 便于后续 MIR lowering 生成显式的 store/写回语义。
    Assign {
        lhs: Expr,
        eq_span: Span,
        rhs: Expr,
    },
    /// `while (cond) { ... }`。
    While {
        cond: Expr,
        body: Block,
    },
    /// `break`（当前阶段不支持 label）。
    Break {
        break_span: Span,
    },
    /// `continue`（当前阶段不支持 label）。
    Continue {
        continue_span: Span,
    },
    Return {
        value: Option<Expr>,
    },
    Todo(&'static str),
}

/// 表达式（expression）。
#[derive(Debug, Clone)]
pub struct Expr {
    pub span: Span,
    pub ty: TypeId,
    pub kind: ExprKind,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Missing,
    Literal(LiteralKind),
    VarRef(ValueRef),
    Block(Block),
    /// closure（lambda）表达式：`{ params -> body }` / `{ body }`。
    ///
    /// 当前阶段（TODO T0710）最小实现：
    /// - 仅保留语法结构与 `ClosureId`；
    /// - 捕获变量布局与 env struct 将在后续任务（TODO T0711）接入。
    Closure(ClosureExpr),
    /// `if (cond) thenExpr else elseExpr?`（表达式形式）。
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    /// `when (subject) { pat -> expr; ... }`（表达式形式）。
    When {
        subject: Box<Expr>,
        arms: Vec<WhenArm>,
    },
    /// 成员访问表达式：`receiver.member`。
    ///
    /// 说明：
    /// - 该节点用于承载 resolver 写回的成员绑定结果（字段/方法/扩展成员的 FQN）；便于后续 MIR lowering/codegen；
    /// - safe-call（`?.`）等更复杂形态将在后续任务中补齐。
    MemberAccess {
        receiver: Box<Expr>,
        member: MemberAccess,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    /// effect operation 调用（spec §5.2/§5.4）。
    ///
    /// 说明：
    /// - 在 AST 中该形态通常表现为 `Effect.op(args...)` 的调用表达式；
    /// - 在后续 lowering（MIR/effect lowering）中会拥有特殊控制流语义（非普通函数调用）。
    Perform {
        op: EffectOpRef,
        args: Vec<CallArg>,
    },
    /// effect handler 表达式：`handle { ... } with { ... }`（spec §5.4）。
    ///
    /// 当前阶段仅承载 non-resuming arms 的结构信息；continuation/resume 相关字段留待后续任务补齐。
    Handle(HandleExpr),
    Todo(&'static str),
}

/// closure（lambda）的 HIR 表示（TODO T0710）。
#[derive(Debug, Clone)]
pub struct ClosureExpr {
    pub span: Span,
    pub id: ClosureId,
    pub params: Vec<Param>,
    pub body: Box<Expr>,
}

/// 一个 effect operation 的“引用”（以 FQN 表示）。
///
/// 说明：该结构主要用于 HIR dump/fixtures 的稳定输出；后续可替换为更结构化的 symbol 引用。
#[derive(Debug, Clone)]
pub struct EffectOpRef {
    pub span: Span,
    pub fqn: String,
}

#[derive(Debug, Clone)]
pub enum LiteralKind {
    Int,
    String,
    Unit,
    Bool(bool),
}

/// 已解析的“值引用”（local/top-level）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueRef {
    Local {
        id: SymbolId,
        name: String,
        decl_span: Span,
    },
    TopLevel {
        id: SymbolId,
        fqn: String,
    },
}

/// 成员访问中的“已解析引用”（来自 resolver 写回的 `ResolvedMemberRef`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberRef {
    Value { id: SymbolId, fqn: String },
    Fun { id: SymbolId, fqn: String },
    ExtensionValue { id: SymbolId, fqn: String },
    ExtensionFun { id: SymbolId, fqn: String },
}

/// 成员访问中的标识符（`receiver.member`）。
///
/// 说明：
/// - `span/name` 用于保持 dump/fixtures 输出稳定且可读；
/// - `resolved` 为空时表示 resolver 未能给出绑定结果（仍保留结构以避免 panic）。
#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub span: Span,
    pub name: String,
    pub resolved: Option<MemberRef>,
}

/// 调用实参（位置参数或命名参数）。
#[derive(Debug, Clone)]
pub enum CallArg {
    Positional(Expr),
    Named {
        name: String,
        name_span: Span,
        value: Expr,
    },
}

/// `when` 的一个分支（arm）：`pat (if guard)? -> body`。
#[derive(Debug, Clone)]
pub struct WhenArm {
    pub span: Span,
    pub pat: WhenPat,
    pub guard: Option<Expr>,
    pub arrow_span: Span,
    pub body: Expr,
}

/// `when` 分支的模式（早期最小子集；后续会与通用 Pattern 统一）。
#[derive(Debug, Clone)]
pub enum WhenPat {
    Else {
        span: Span,
    },
    /// `_`：通配符模式（匹配任意值）。
    Wildcard {
        span: Span,
    },
    /// rest：`..`（忽略剩余字段/元素；仅允许出现在 tuple/variant pattern 内）。
    Rest {
        span: Span,
    },
    /// `is Type`。
    Is {
        span: Span,
        ty: TypeId,
    },
    /// 绑定变量模式：`x`（把匹配到的值绑定到变量 `x`）。
    Bind {
        span: Span,
        id: SymbolId,
        name: String,
    },
    /// tuple 模式：`(p1, p2, ...)`。
    Tuple {
        span: Span,
        elements: Vec<WhenPat>,
    },
    /// enum variant 模式：`Some(x)` / `None`（0 参数 variant）。
    Variant {
        span: Span,
        name_span: Span,
        name: String,
        args: Vec<WhenPat>,
    },
    IntLit {
        span: Span,
    },
    StringLit {
        span: Span,
    },
    BoolLit {
        span: Span,
        value: bool,
    },
}

/// `handle` 表达式（HIR 视图）。
#[derive(Debug, Clone)]
pub struct HandleExpr {
    pub body: Block,
    pub arms: Vec<HandleArm>,
    pub finally: Option<Block>,
}

/// `handle` 的一个 handler arm（HIR 视图）。
#[derive(Debug, Clone)]
pub struct HandleArm {
    pub span: Span,
    pub op: HandleOp,
    pub body: Expr,
}

/// handler arm head 中的 effect operation：`Effect.op(binders...)`（HIR 视图）。
#[derive(Debug, Clone)]
pub struct HandleOp {
    pub span: Span,
    pub effect_ty: TypeId,
    pub op: EffectOpRef,
    pub binders: Vec<HandleBinder>,
}

/// handler arm 的一个参数 binder：`name` 或 `name: Type`（HIR 视图）。
#[derive(Debug, Clone)]
pub struct HandleBinder {
    pub span: Span,
    pub id: SymbolId,
    pub name: String,
    pub ty: TypeId,
}
