//! Exact provisional-object verification for strong callable registrations.

mod error;
pub use error::*;

pub(in crate::link_object) mod object_definition;
pub use object_definition::ObjectDefinitionRelocationFailureV1;
mod physical;
pub(in crate::link_object) mod record;

mod fingerprints;
pub use fingerprints::*;

mod body_fingerprints;
pub use body_fingerprints::*;

mod registration_fingerprints;
pub use registration_fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
