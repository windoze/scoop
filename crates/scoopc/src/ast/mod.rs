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
}

#[derive(Debug, Clone)]
pub struct FunDecl {
    pub span: Span,
    pub name: Ident,
    pub params_span: Span,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ident {
    pub span: Span,
}

