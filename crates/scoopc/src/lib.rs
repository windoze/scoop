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

/// Migration anchor for MIR stage facts.
///
/// New stage/fact crates should depend on `scoopc_mir_facts` directly; this
/// facade re-export exists only for the current monolithic compiler crate.
pub use scoopc_mir_facts as mir_facts;

/// Migration anchor for the independent effect/control fact product.
///
/// New stage/fact crates should depend on `scoopc_effect_facts` directly; this
/// facade re-export exists only while the current effect-facts builder still
/// lives in the monolithic compiler crate.
pub use scoopc_effect_facts as effect_facts_product;

/// Migration anchor for the independent LIR fact product.
///
/// New stage/fact crates should depend on `scoopc_lir_facts` directly; this
/// facade re-export exists while LIR construction still lives in the monolithic
/// compiler crate.
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
pub(crate) mod intrinsics {
    pub(crate) use scoopc_hir::intrinsics::*;
}
pub use scoopc_hir::itable;
pub use scoopc_mir::mir;
pub use scoopc_mir::monomorph;
pub mod opt;
pub use scoopc_ast::parser;
pub mod pipeline;
pub use scoopc_hir::resolve;
pub use scoopc_hir::session;
pub use scoopc_mir::rtti;
pub mod source;
pub mod span;
pub use scoopc_mir::stable_id;
#[path = "../../scoopc_codegen_llvm/src/stackmap.rs"]
pub mod stackmap;
pub use scoopc_ast::syntax;
pub use scoopc_hir::sysroot;
pub use scoopc_hir::target;
pub mod ty;
pub use scoopc_hir::typecheck;
pub use scoopc_hir::vtable;
pub use scoopc_hir::warnings;

/// LLVM 后端（inkwell）。
///
/// 注意：该模块需要启用 `scoopc` 的 `llvm` feature（默认关闭）。
#[cfg(feature = "llvm")]
#[path = "../../scoopc_codegen_llvm/src/llvm/mod.rs"]
pub mod llvm;

#[cfg(test)]
mod audit;

#[cfg(test)]
mod pipeline_gap_audit;

#[cfg(test)]
mod pipeline_user_visible_failure_policy;
