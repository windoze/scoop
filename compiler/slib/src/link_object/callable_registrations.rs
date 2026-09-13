//! Exact provisional-object verification for strong callable registrations.

mod error;
pub use error::*;

mod physical;
mod record;

mod fingerprints;
pub use fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
