//! Exact provisional-object verification for strong static-storage registrations.

mod digest;
mod error;
mod fingerprints;
mod physical;
pub(in crate::link_object) mod record;
mod relocations;
mod shape_fingerprints;
mod storage_fingerprints;
mod verification;

pub use error::*;
pub use fingerprints::*;
pub use shape_fingerprints::*;
pub use storage_fingerprints::*;
pub use verification::*;

#[cfg(test)]
mod tests;
