//! Safepoint registration data and fingerprint calculation.

pub(in crate::link_object) mod record;

mod fingerprints;
pub use fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
