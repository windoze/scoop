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

mod closure_limits;
pub use closure_limits::*;

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

mod cross_cone_closure;
pub use cross_cone_closure::*;

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
