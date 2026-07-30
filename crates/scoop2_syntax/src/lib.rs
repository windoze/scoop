//! Scoop 下一代前端：词法、语法与 AST。
//!
//! 本 crate 覆盖编译管线的 `source text → tokens → AST` 部分：
//!
//! - [`token`]：token 定义；
//! - [`lexer`]：手写词法分析器，产出完整 token 流并附带字面量校验诊断；
//! - [`ast`]：完整 AST 定义（`NodeId` + `Symbol` + `Span`）；
//! - [`parser`]：手写递归下降 + Pratt 表达式解析器，带 item/stmt 级错误恢复；
//! - [`dump`]：稳定 AST 文本渲染（`dump-ast` 的 golden 格式）。
//!
//! 语法事实来源是 `docs/spec/grammar.md`；parser 逐条对应文法产生式。

#![forbid(unsafe_code)]

pub mod ast;
pub mod dump;
pub mod lexer;
pub mod parser;
pub mod token;

pub use scoop2_base as base;
