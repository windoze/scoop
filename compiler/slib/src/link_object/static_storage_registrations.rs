//! Static-storage registration slots and semantic fingerprints.

mod binding;
pub(in crate::link_object) mod record;
mod shape_fingerprints;
mod strong_fingerprints;

pub use binding::*;
pub use shape_fingerprints::*;
pub use strong_fingerprints::*;
