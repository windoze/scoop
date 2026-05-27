//! Scoop compiler umbrella crate（`scoopc`）
//!
//! 本 crate 负责：
//! - 提供 base / fact / stage crate 的 facade re-export；
//! - 保留 `frontend`、`pipeline`、`session` 与 driver 编排 helper；
//! - 通过 `llvm` facade 转发到 standalone LLVM backend crate。
//!
//! `scoop`（driver）crate 只负责命令行与调度。

extern crate self as scoopc;

/// Migration anchors for stage-independent base crates.
///
/// `scoopc::{span, source, ty}` are re-export adapters over their base crates;
/// `stable_id` re-exports base identity primitives while keeping type-aware
/// stable-key helpers on this facade for the current monolithic pipeline. New
/// stage/fact crates should depend on the base crates directly instead of
/// depending on this facade.
pub mod base {
    pub use scoop_project_model as project_model;
    pub use scoopc_ids as ids;
    pub use scoopc_source as source;
    pub use scoopc_span as span;
    pub use scoopc_types as types;
}

/// Migration anchor for HIR semantic facts.
///
/// New stage/fact crates should depend on `scoopc_hir_facts` directly; this
/// facade re-export exists only for the current monolithic compiler crate.
pub use scoopc_hir_facts as hir_facts;

/// Migration anchor for MIR stage facts.
///
/// New stage/fact crates should depend on `scoopc_mir_facts` directly; this
/// facade re-export exists only for the current monolithic compiler crate.
pub use scoopc_mir_facts as mir_facts;

/// Migration anchor for the independent effect/control fact product.
///
/// New stage/fact crates should depend on `scoopc_effect_facts` directly; this
/// facade re-export exists only for umbrella compatibility.
pub use scoopc_effect_facts as effect_facts_product;

/// Migration anchor for the independent LIR fact product.
///
/// New stage/fact crates should depend on `scoopc_lir_facts` directly; this
/// facade re-export exists only for umbrella compatibility.
pub use scoopc_lir_facts as lir_facts_product;

pub use scoopc_ast as ast;
pub mod cone;
pub mod driver_cli;
pub(crate) mod dump_support {
    pub(crate) use scoopc_hir::dump_support::*;
}
pub use scoopc_effect_facts_stage as effect_facts_stage;
pub use scoopc_lir::effect_facts;
pub use scoopc_lir::effect_lowered;
pub mod frontend;
pub use scoopc_hir::hir;
pub use scoopc_hir::infer;
pub use scoopc_hir::itable;
pub use scoopc_mir::mir;
pub use scoopc_mir::monomorph;
pub mod native_build;
pub mod opt;
pub use scoopc_ast::parser;
pub mod pipeline;
pub use scoopc_hir::resolve;
pub use scoopc_hir::session;
pub use scoopc_mir::rtti;
pub mod single_cone;
pub mod source;
pub mod span;
pub use scoopc_ast::syntax;
pub use scoopc_codegen_llvm::stackmap;
pub use scoopc_hir::sysroot;
pub use scoopc_hir::target;
pub use scoopc_mir::stable_id;
pub mod tool_commands;
pub mod ty;
pub use scoopc_hir::typecheck;
pub use scoopc_hir::vtable;
pub use scoopc_hir::warnings;

/// LLVM 后端 facade。
///
/// 注意：该模块需要启用 `scoopc` 的 `llvm` feature（默认启用，可用 `--no-default-features` 关闭）。
#[cfg(feature = "llvm")]
pub mod llvm;

#[cfg(test)]
mod audit;

#[cfg(test)]
mod pipeline_gap_audit;

#[cfg(test)]
mod pipeline_user_visible_failure_policy;
