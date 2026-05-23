//! HIR semantic frontend stage for the Scoop compiler.
//!
//! This crate owns the AST-to-HIR semantic barrier: name resolution,
//! typechecking/inference, HIR lowering, and frontend-owned dispatch metadata.

#![forbid(unsafe_code)]

pub mod cone {
    pub use scoopc_project_model::*;

    pub mod manifest;
    pub mod package;
}

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
pub use scoopc_hir_facts as hir_facts;

pub mod source {
    pub use scoopc_source::*;
}

pub mod span {
    pub use scoopc_span::*;
}

pub mod ty {
    pub use scoopc_types::*;
}

pub mod opt {
    pub use scoopc_project_model::OptLevel;
}

pub mod dump_support;
pub mod expr_facts;
pub mod hir;
pub mod infer;
pub mod intrinsics;
pub mod itable;
pub mod monomorph;
pub mod resolve;
pub mod session;
pub mod stable_id;
pub mod stage;
pub mod sysroot;
pub mod target;
pub mod typecheck;
pub mod vtable;
pub mod warnings;

pub(crate) mod hir_completeness;
