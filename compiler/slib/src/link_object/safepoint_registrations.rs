//! Exact provisional-object verification for strong safepoint registrations.

mod error;
pub use error::*;

pub(in crate::link_object) mod physical;
pub(in crate::link_object) mod record;

mod fingerprints;
pub use fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
