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
    pub body: Option<Block>,
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
    /// 初始化表达式的源代码范围（当前阶段不解析表达式，只保留 span）。
    pub init: Option<Span>,
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
