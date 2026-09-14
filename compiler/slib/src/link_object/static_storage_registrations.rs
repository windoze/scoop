//! Exact provisional-object verification for strong static-storage registrations.

mod digest;
mod error;
mod physical;
pub(in crate::link_object) mod record;
mod relocations;
mod verification;

pub use error::*;
pub use verification::*;

#[cfg(test)]
mod tests;
