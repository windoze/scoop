//! Scoop 下一代前端：名称解析、类型检查与 typed HIR。
//!
//! 本 crate 覆盖编译管线的 `AST → typed HIR` 部分：
//!
//! - [`resolve`]：包/导入/可见性/重载集/块作用域的名称解析，多文件 cone 支持；
//! - [`ty`]：类型存储（`TypeId` interning）与类型系统核心；
//! - [`typecheck`]：声明头与表达式/语句的完整类型检查与推断；
//! - [`hir`]：typed HIR 数据结构与 AST→HIR lowering（全部脱糖在此完成）；
//! - [`completeness`]：typed HIR 交付前的完整性闸门（拒绝任何未解析引用/缺类型节点）。
//!
//! 阶段顺序：`parse → resolve(headers) → resolve(bodies) → typecheck → lower → verify`。

#![forbid(unsafe_code)]

pub use scoop2_base as base;
pub use scoop2_syntax as syntax;

pub mod completeness;
pub mod hir;
pub mod resolve;
pub mod ty;
pub mod typecheck;
