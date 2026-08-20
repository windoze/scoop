//! Exact provisional-object verification for strong static-storage registrations.

mod error;
mod physical;
pub(in crate::link_object) mod record;
mod relocations;
mod shape_fingerprints;
mod strong_fingerprints;
mod verification;

pub use error::*;
pub use shape_fingerprints::*;
pub use strong_fingerprints::*;
pub use verification::*;

#[cfg(test)]
mod tests;
