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
use scoop_ast::{Diagnostic, Span};

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
    /// Non-fatal source diagnostics produced while constructing this output.
    /// They are not part of either HIR graph or serialized metadata.
    pub warnings: Vec<Diagnostic>,
}

mod ids;
pub use ids::*;

mod types;
pub use types::*;

mod integer;
pub use integer::*;

mod entities;
pub use entities::*;

mod declarations;
pub use declarations::*;

mod body;
pub use body::*;

mod bindings;
pub use bindings::*;

mod iteration;
pub use iteration::*;

mod imports;
pub use imports::*;
mod intrinsics;
pub use intrinsics::*;

mod persistent;
pub use persistent::*;
mod visibility;
pub use visibility::*;
pub mod export_surface;
pub mod wire;
pub use export_surface::*;

mod source_interfaces;
pub use source_interfaces::*;

mod dump;
pub use dump::{dump, dump_pattern};

/// The product kind of one Cone's output, closed over the entry link
/// (DESIGN section 1.3): a library has no entry linkage at all, and an
/// executable's entry is structurally present, never an optional field
/// that could disagree with the manifest kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConeOutputKind {
    Library,
    Executable { local_entry: FunctionId },
}
