//! AST（抽象语法树）。
//!
//! 目前阶段的 AST 目标：
//! - 足够表达“文件头（package/import）+ 顶层声明（fun/val/var 等）”的结构
//! - 节点主要用 `Span` 指回源文本，避免早期过度分配
//!
//! 注意：随着 parser/typechecker 完善，AST 结构可能会演进。

use crate::span::Span;

#[derive(Debug, Clone)]
pub struct File {
    pub package: Option<PackageDecl>,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub struct PackageDecl {
    pub span: Span,
    pub path: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub span: Span,
    pub path: Vec<Ident>,
    pub has_star: bool,
}

#[derive(Debug, Clone)]
pub enum Item {
    Fun(FunDecl),
    Type(TypeDecl),
    Val(ValDecl),
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub span: Span,
    pub kind: TypeKind,
    pub name: Ident,
    /// 类型体（`{ ... }`）。
    ///
    /// 当前阶段：
    /// - parser 仍可能仅保证括号平衡与 span 正确
    /// - 成员列表的解析会在后续任务中逐步补齐
    pub body: Option<TypeBody>,
}

/// 类型体（`{ ... }`）——可包含成员列表。
///
/// 注意：这里与 `Block` 不同：
/// - `Block` 用于函数体/表达式块（后续会包含语句）
/// - `TypeBody` 用于 `class/interface/struct/enum/effect` 的成员声明列表
#[derive(Debug, Clone)]
pub struct TypeBody {
    pub span: Span,
    pub members: Vec<TypeMember>,
}

/// 类型体中的成员声明（最小骨架）。
#[derive(Debug, Clone)]
pub enum TypeMember {
    Val(ValDecl),
    Fun(FunDecl),
    Type(TypeDecl),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
}

#[derive(Debug, Clone)]
pub struct FunDecl {
    pub span: Span,
    pub name: Ident,
    pub params_span: Span,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
    pub body: FunBody,
}

#[derive(Debug, Clone)]
pub enum FunBody {
    Block(Block),
    Missing,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub span: Span,
}

/// 表达式（最小子集）。
///
/// 说明：当前阶段只需要支撑 initializer 与函数体解析的增量推进，因此先保留一个非常小的集合。
/// 后续任务会逐步补齐调用、成员访问、二元运算、控制流等表达式节点。
#[derive(Debug, Clone)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    /// 解析失败或尚未实现时的占位节点（保持 span 以便诊断/回归）。
    Missing,
    Ident(Ident),
    IntLit,
    StringLit,
    Block(Block),
}

impl Expr {
    pub fn missing(span: Span) -> Self {
        Self {
            span,
            kind: ExprKind::Missing,
        }
    }
}

/// 语句（最小骨架）。
///
/// 目前阶段仅为后续 block 解析预留结构；T0207/T0208 会逐步扩展其子集。
#[derive(Debug, Clone)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Empty,
    Expr(Expr),
    Missing,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Ident,
    pub ty: Option<TypeRef>,
}

#[derive(Debug, Clone)]
pub struct ValDecl {
    pub span: Span,
    pub kind: ValKind,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    /// 初始化表达式（当前阶段可能为 `ExprKind::Missing`，后续任务会逐步补齐解析）。
    pub init: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValKind {
    Val,
    Var,
}

#[derive(Debug, Clone)]
pub enum TypeRef {
    Path(TypePath),
    Tuple(TypeTuple),
    Nullable {
        span: Span,
        inner: Box<TypeRef>,
    },
}

#[derive(Debug, Clone)]
pub struct TypePath {
    pub span: Span,
    pub segments: Vec<Ident>,
    pub args: Vec<TypeRef>,
}

#[derive(Debug, Clone)]
pub struct TypeTuple {
    pub span: Span,
    pub elements: Vec<TypeRef>,
}

impl TypeRef {
    pub fn span(&self) -> Span {
        match self {
            TypeRef::Path(p) => p.span,
            TypeRef::Tuple(t) => t.span,
            TypeRef::Nullable { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ident {
    pub span: Span,
}
