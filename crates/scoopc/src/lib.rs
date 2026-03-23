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
pub mod hir;
pub mod infer;
pub mod parser;
pub mod resolve;
pub mod session;
pub mod source;
pub mod span;
pub mod syntax;
pub mod sysroot;
pub mod ty;
pub mod typecheck;
