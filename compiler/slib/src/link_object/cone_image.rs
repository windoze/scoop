//! Exact provisional-object verification for the per-Cone runtime image.

mod error;
pub use error::*;

mod physical;
mod record;
mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
