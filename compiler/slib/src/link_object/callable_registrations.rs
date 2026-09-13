//! Exact provisional-object verification for strong callable registrations.

mod error;
pub use error::*;

mod object_definition;
mod physical;
mod record;

mod fingerprints;
pub use fingerprints::*;

mod body_fingerprints;
pub use body_fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
