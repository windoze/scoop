//! Exact provisional-object verification for the per-Cone runtime image.

mod error;
pub use error::*;

mod fingerprint;
mod physical;
mod record;
pub use fingerprint::*;
mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
