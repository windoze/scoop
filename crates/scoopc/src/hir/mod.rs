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

use crate::span::Span;
use crate::ty::TypeId;

pub use lower::{HirLowerError, LoweredHir, lower_for_dump};

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
    pub name: String,
    pub ty: TypeId,
}

/// `val`/`var` 声明（顶层或局部）。
#[derive(Debug, Clone)]
pub struct ValDecl {
    pub span: Span,
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
    Return { value: Option<Expr> },
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
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    Todo(&'static str),
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
    Local { name: String, decl_span: Span },
    TopLevel { fqn: String },
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
