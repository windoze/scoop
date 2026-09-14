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

mod compile_decode;
pub use compile_decode::*;

mod link_decode;
pub use link_decode::*;

mod metadata;
pub use metadata::*;

mod foundation;
pub use foundation::*;

mod semantic;
pub use semantic::*;

mod link_object;
pub use link_object::*;

mod diagnostic;
pub use diagnostic::*;
