//! Exact provisional-object verification for strong immortal-object registrations.

mod error;
pub use error::*;

mod digest;
mod physical;
pub(in crate::link_object) mod record;
mod relocations;

mod fingerprints;
pub use fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
