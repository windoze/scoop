//! HIR definitions and HIR meta: the data channel between HIR and MIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.2 and
//! `docs/milestone4/DESIGN.md` section 3.2.
//!
//! Structural completeness rules (see AGENTS.md): every expression
//! carries its resolved type (`Expr::ty`), every call carries its
//! resolved target and type arguments, patterns carry resolved
//! variant/field indices and binding locals, and a module always has
//! an entry point (`Module::entry`).

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub mod concrete;

/// Cross-Cone semantic interface and generic-template graph.  This name makes
/// the consumer boundary explicit without changing the export-side data model.
pub type ExportHir = Module;

/// Fully instantiated graph consumed only by the current Cone's MIR stage.
pub type LocalConcreteHir = concrete::Module;

/// HIR has two structurally isolated products for two different consumers.
/// Export ids and local-concrete ids belong to separate Rust type families and
/// therefore cannot cross the boundary accidentally.
#[derive(Debug, Clone)]
pub struct Output {
    pub export: ExportHir,
    pub local: LocalConcreteHir,
}

mod ids;
pub use ids::*;

mod types;
pub use types::*;

mod entities;
pub use entities::*;

mod declarations;
pub use declarations::*;

mod body;
pub use body::*;

mod intrinsics;
pub use intrinsics::*;

mod visibility;
pub use visibility::*;

mod source_interfaces;
pub use source_interfaces::*;

mod dump;
pub use dump::{dump, dump_pattern};
