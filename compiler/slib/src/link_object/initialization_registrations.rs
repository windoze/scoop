//! Exact provisional-object verification and digest production for strong
//! initialization-unit registrations.

mod error;
mod physical;
pub(in crate::link_object) mod record;
mod relocations;
mod strong_fingerprints;
mod verification;

pub use error::*;
pub use strong_fingerprints::*;
pub use verification::*;

#[cfg(test)]
mod tests;
