//! `.slib` packaging and reader: bundles the `.o` with HIR / MIR / LIR
//! metadata for downstream Cones.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.6.

mod member;
pub use member::*;

mod archive;
pub use archive::*;
