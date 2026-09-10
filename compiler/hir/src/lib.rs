//! HIR definitions and HIR meta: the data channel between HIR and MIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.2 and
//! `docs/milestone4/DESIGN.md` section 3.2.
//!
//! Structural completeness rules (see AGENTS.md): every expression
//! carries its resolved type (`Expr::ty`), every call carries its
//! resolved target and type arguments, patterns carry resolved
//! variant/field indices and binding locals. Executable entry identity is
//! carried only by the temporary, explicitly legacy wrappers below; library
//! and frontend HIR modules do not require one.

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

mod legacy;
pub use legacy::*;

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

mod intrinsics;
pub use intrinsics::*;

mod visibility;
pub use visibility::*;

mod source_record;
pub use source_record::*;

mod native_boundary;
pub use native_boundary::*;

mod persistent_nominals;
pub use persistent_nominals::*;

mod persistent_properties;
pub use persistent_properties::*;

mod persistent_accessors;
pub use persistent_accessors::*;

mod foundation;
pub use foundation::*;

mod source_interfaces;
pub use source_interfaces::*;

mod dump;
#[doc(hidden)]
pub use dump::HirDumpInput;
pub use dump::{dump, dump_legacy_executable, dump_pattern};
