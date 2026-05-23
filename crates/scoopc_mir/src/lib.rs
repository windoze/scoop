//! MIR stage for the Scoop compiler.
//!
//! This crate owns backend-neutral MIR lowering, monomorphic materialization,
//! MIR pass artifacts, RTTI helpers, and the MIR-facing stable-symbol facade.

#![forbid(unsafe_code)]

pub mod base {
    pub use scoopc_ids as ids;
    pub use scoopc_project_model as project_model;
    pub use scoopc_source as source;
    pub use scoopc_span as span;
    pub use scoopc_types as types;
}

pub use scoopc_ast as ast;
pub use scoopc_ast::parser;
pub use scoopc_ast::syntax;
pub use scoopc_hir::hir;
pub use scoopc_hir::infer;
pub use scoopc_hir::itable;
pub use scoopc_hir::resolve;
pub use scoopc_hir::session;
pub use scoopc_hir::sysroot;
pub use scoopc_hir::target;
pub use scoopc_hir::typecheck;
pub use scoopc_hir::vtable;
pub use scoopc_hir::warnings;
pub use scoopc_hir_facts as hir_facts;
pub use scoopc_mir_facts as mir_facts;

pub mod cone {
    pub use scoopc_project_model::*;
}

pub(crate) mod dump_support {
    pub(crate) use scoopc_hir::dump_support::*;
}

pub(crate) mod expr_facts {
    pub(crate) use scoopc_hir::expr_facts::*;
}

pub mod monomorph;
pub mod opt {
    pub use scoopc_project_model::{InvalidOptLevel, OptLevel};
}
pub mod source {
    pub use scoopc_source::*;
}
pub mod span {
    pub use scoopc_span::*;
}
pub mod stable_id;
pub mod ty {
    pub use scoopc_types::*;
}

pub mod mir;
pub mod rtti;
