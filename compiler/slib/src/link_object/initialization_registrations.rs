//! Exact provisional-object verification and digest production for strong
//! initialization-unit registrations.

mod digest;
mod error;
mod fingerprints;
mod physical;
pub(in crate::link_object) mod record;
mod relocations;
mod verification;

pub use error::*;
pub use fingerprints::*;
pub use verification::*;

#[cfg(test)]
mod tests;
