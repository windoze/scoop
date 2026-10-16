//! Immortal-object registration identities and canonical definition ABIs.

pub(in crate::link_object) mod record;
mod registration_fingerprints;
pub use registration_fingerprints::*;

#[cfg(test)]
mod tests;
