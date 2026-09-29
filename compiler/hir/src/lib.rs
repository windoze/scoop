//! HIR definitions and HIR meta: the data channel between HIR and MIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.2 and
//! `docs/milestone4/DESIGN.md` section 3.2.
//!
//! Structural completeness rules (see AGENTS.md): every expression
//! carries its resolved type (`Expr::ty`), every call carries its
//! resolved target and type arguments, patterns carry resolved
//! variant/field indices and binding locals. Every successful HIR product
//! carries a closed library/executable branch in both the export and local-
//! concrete id domains; an executable entry is never optional.

use la_arena::{Arena, Idx};
use scoop_ast::Diagnostic;
pub use scoop_ast::Span;

pub mod concrete;

/// Cross-Cone semantic interface and generic-template graph.  This name makes
/// the consumer boundary explicit without changing the export-side data model.
pub type ExportHir = Module;

/// Fully instantiated graph consumed only by the current Cone's MIR stage.
pub type LocalConcreteHir = concrete::Module;

/// HIR has two structurally isolated products for two different consumers.
/// Export ids and local-concrete ids belong to separate Rust type families and
/// therefore cannot cross the boundary accidentally. The constructor is the
/// only public way to pair them, so their output branches cannot disagree.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Output {
    pub export: ExportHirOutput,
    pub local: LocalConcreteHirOutput,
    /// Exact native-boundary witness derived from both the source graph and
    /// the concrete callback-application graph.
    pub native_boundary_types: HirNativeBoundaryTypeDefinitions,
    /// Non-fatal source diagnostics produced while constructing this output.
    /// They are not part of either HIR graph or serialized metadata.
    pub warnings: Vec<Diagnostic>,
}

mod output;
pub use output::*;

mod output_kind;
pub use output_kind::*;

mod imported_dependency;
pub use imported_dependency::*;

mod initialization_dependencies;
pub use initialization_dependencies::{HirInitializationUseError, HirPropertyInitializationUseV1};

mod production;
pub use production::*;

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

mod source_call_receiver;
pub use source_call_receiver::*;

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

mod persistent_constructors;
pub use persistent_constructors::*;

mod persistent_functions;
pub use persistent_functions::*;

mod persistent_callbacks;
pub use persistent_callbacks::*;

mod persistent_dispatch;
pub use persistent_dispatch::*;
mod persistent_export_bindings;
pub use persistent_export_bindings::*;

mod reexports;
pub use reexports::*;

mod cross_cone_interface;
pub use cross_cone_interface::*;

mod cross_cone_type_semantics;
pub use cross_cone_type_semantics::*;

mod persistent_local_bindings;
pub use persistent_local_bindings::*;

mod persistent_source_contexts;
pub use persistent_source_contexts::*;

mod persistent_native_contracts;
pub use persistent_native_contracts::*;

mod persistent_native_boundary;
pub use persistent_native_boundary::*;

mod persistent_definition_origins;
pub use persistent_definition_origins::*;

mod persistent_aliases;
pub use persistent_aliases::*;

mod persistent_enum_members;
pub use persistent_enum_members::*;

mod persistent_fields;
pub use persistent_fields::*;

mod persistent_object_values;
pub use persistent_object_values::*;

mod persistent_initialization_units;
pub use persistent_initialization_units::*;

mod persistent_types;
pub use persistent_types::*;

mod foundation;
pub use foundation::*;

mod semantic_world;
pub use semantic_world::*;

mod default_local_values;
pub use default_local_values::*;

mod source_interfaces;
pub use source_interfaces::*;

mod dump;
pub use dump::{dump, dump_module, dump_pattern};

#[cfg(test)]
mod nominal_interface_fixture;
