//! Scoop 编译器核心库（`scoopc`）
//!
//! 本 crate 负责：
//! - 读取源文件、管理 span/位置等基础设施
//! - 词法/语法分析（后续阶段）
//! - 名字解析、类型检查、效果系统检查（后续阶段）
//! - HIR/MIR lowering 与 LLVM(inkwell) codegen（后续阶段）
//!
//! `scoop`（driver）crate 只负责命令行与调度。

pub mod ast;
pub mod comptime;
pub mod cone;
pub(crate) mod devirtualize;
pub(crate) mod effect_analysis;
#[cfg(not(feature = "llvm"))]
pub(crate) mod effect_step_summary;
pub(crate) mod expr_facts;
pub mod hir;
pub mod infer;
pub mod itable;
pub mod mir;
pub mod monomorph;
pub mod opt;
pub mod parser;
pub(crate) mod program_facts;
pub mod resolve;
pub mod rtti;
pub mod session;
pub mod source;
pub mod span;
pub mod stackmap;
pub mod syntax;
pub mod sysroot;
pub mod target;
pub mod ty;
pub mod typecheck;
pub mod vtable;
pub mod warnings;

/// LLVM 后端（inkwell）。
///
/// 注意：该模块需要启用 `scoopc` 的 `llvm` feature（默认关闭）。
#[cfg(feature = "llvm")]
pub mod llvm;
