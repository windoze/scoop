//! Exact provisional-object verification for strong type registrations.

mod error;
pub use error::*;

mod digest;
mod physical;
pub(in crate::link_object) mod record;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
