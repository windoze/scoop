//! Scoop 下一代前端（scoop2）的阶段无关基础设施。
//!
//! 本 crate 是所有 scoop2 阶段 crate 的公共基座，提供：
//!
//! - [`Span`]：单源文件内的 UTF-8 字节偏移区间；
//! - [`SourceFile`] / [`FileId`]：源文件身份、行表与行列映射；
//! - [`Symbol`] / [`Interner`]：字符串 interning，AST/HIR 中的标识符一律使用 `Symbol`；
//! - [`NodeId`]：AST 节点身份，语义阶段的致密侧表以它为键；
//! - [`diag`]：数据驱动的诊断表示（稳定诊断码 + span + help）与纯文本渲染器。
//!
//! 本 crate 不依赖任何其他编译器 crate，也不依赖 miette/thiserror：
//! 诊断渲染由本 crate 内的手写 renderer 完成，输出格式稳定、可被 fixture
//! runner 的正则（诊断码、`line:col`）提取。

#![forbid(unsafe_code)]

pub mod diag;
mod node;
mod source;
mod span;
mod symbol;

pub use node::{NodeId, NodeIdAllocator};
pub use source::{FileId, SourceFile, SourceOrigin, SourceTrust};
pub use span::Span;
pub use symbol::{Interner, Symbol};
