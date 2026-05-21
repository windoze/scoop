//! Scoop 编译器核心库（`scoopc`）
//!
//! 本 crate 负责：
//! - 读取源文件、管理 span/位置等基础设施
//! - 词法/语法分析（后续阶段）
//! - 名字解析、类型检查、效果系统检查（后续阶段）
//! - HIR/MIR lowering 与 LLVM(inkwell) codegen（后续阶段）
//!
//! `scoop`（driver）crate 只负责命令行与调度。

/// Migration anchors for stage-independent base crates.
///
/// `scoopc::{span, source, ty}` are re-export adapters over their base crates;
/// `stable_id` re-exports base identity primitives while keeping type-aware
/// stable-key helpers on this facade for the current monolithic pipeline. New
/// stage/fact crates should depend on the base crates directly instead of
/// depending on this facade.
pub mod base {
    pub use scoopc_ids as ids;
    pub use scoopc_project_model as project_model;
    pub use scoopc_source as source;
    pub use scoopc_span as span;
    pub use scoopc_types as types;
}

/// Migration anchor for HIR semantic facts.
///
/// New stage/fact crates should depend on `scoopc_hir_facts` directly; this
/// facade re-export exists only for the current monolithic compiler crate.
pub use scoopc_hir_facts as hir_facts;

pub mod ast;
pub mod cone;
pub(crate) mod devirtualize;
pub mod driver_cli;
pub(crate) mod dump_support;
pub(crate) mod effect;
pub mod effect_facts;
pub mod effect_lowered;
pub(crate) mod expr_facts;
pub mod frontend;
pub mod hir;
pub mod infer;
pub(crate) mod intrinsics;
pub mod itable;
pub mod mir;
pub mod monomorph;
pub mod opt;
pub mod parser;
pub mod pipeline;
pub mod resolve;
pub mod rtti;
pub mod session;
pub mod source;
pub(crate) mod source_site_migration_facts;
pub mod span;
pub mod stable_id;
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

#[cfg(test)]
mod audit;

#[cfg(test)]
mod pipeline_gap_audit;

#[cfg(test)]
mod pipeline_user_visible_failure_policy;
