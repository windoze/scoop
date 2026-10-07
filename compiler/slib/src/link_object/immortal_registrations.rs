//! Exact provisional-object verification for strong immortal-object registrations.

mod error;
pub use error::*;

mod object;
mod physical;
pub(in crate::link_object) mod record;
mod relocations;

mod registration_fingerprints;
pub use registration_fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
