//! Exact provisional-object verification for strong type registrations.

mod error;
pub use error::*;

mod physical;
pub(in crate::link_object) mod record;
mod targets;
mod versioned;
pub(in crate::link_object) use versioned::{
    LinkDescriptorReference, LinkDispatchCallableReference,
};

mod dependency_fingerprints;
pub use dependency_fingerprints::*;

mod registration_fingerprints;
pub use registration_fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
