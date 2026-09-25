//! `.slib` packaging and reader: bundles the `.o` with HIR / MIR / LIR
//! metadata for downstream Cones.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.6.

mod member;
pub use member::*;

mod archive;
pub use archive::*;

mod profile;
pub use profile::*;

mod compatibility;
pub use compatibility::*;

mod manifest;
pub use manifest::*;

mod envelope;
pub use envelope::*;

mod graph;
pub use graph::*;

mod prebuilt_summary;
pub use prebuilt_summary::*;

mod dual_artifact;
pub use dual_artifact::*;

mod compile_decode;
pub use compile_decode::*;

mod compile_sections;
pub use compile_sections::CompileSectionDecodeError;

mod link_decode;
pub use link_decode::*;

mod strong_compile_decode;
pub use strong_compile_decode::*;

mod cross_cone_compile_decode;
pub use cross_cone_compile_decode::*;

mod layout_compile_decode;
pub use layout_compile_decode::*;

mod dependency_reachability;
mod layout_compile_closure;
pub use layout_compile_closure::*;

mod layout_link_objects;
pub use layout_link_objects::{LayoutLinkObjectContentsError, ReplayedLayoutLinkObjectContentsV1};

mod layout_link_symbols;
pub use layout_link_symbols::{LayoutLinkSymbolUseError, ReplayedLayoutLinkSymbolUsesV1};

mod cross_cone_closure;
pub use cross_cone_closure::*;

mod cross_cone_hir_authority;
mod hir_dependency_calls;
mod hir_interface_validation;
pub use cross_cone_hir_authority::{
    CrossConeHirIntrinsicTypeError, CrossConeHirNominalAuthorityError,
    CrossConeIntrinsicDeclarationError,
};
pub use hir_interface_validation::{
    CrossConeHirCallSiteOriginError, CrossConeHirReferenceSurfaceError, CrossConeHirTypeSiteError,
};

mod publish;
pub use publish::*;

mod metadata;
pub use metadata::*;

mod foundation;
pub use foundation::*;

mod strong_artifact;
pub use strong_artifact::*;

mod semantic;
pub use semantic::*;

mod link_object;
pub use link_object::*;

mod diagnostic;
pub use diagnostic::*;

#[cfg(test)]
mod nominal_interface_fixture;
