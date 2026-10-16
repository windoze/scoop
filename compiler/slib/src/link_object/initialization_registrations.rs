//! Initialization registration slots and gateway fingerprints.

mod binding;
pub(in crate::link_object) mod record;
mod strong_fingerprints;
pub use binding::*;
pub use strong_fingerprints::*;

#[cfg(test)]
mod tests;
