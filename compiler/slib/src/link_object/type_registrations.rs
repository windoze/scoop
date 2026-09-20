//! Exact provisional-object verification for strong type registrations.

mod error;
pub use error::*;

mod digest;
mod physical;
pub(in crate::link_object) mod record;
mod versioned;
pub(in crate::link_object) use versioned::{
    LinkDescriptorReference, LinkDispatchCallableReference,
};

mod fingerprints;
pub use fingerprints::*;

mod dependency_fingerprints;
pub use dependency_fingerprints::*;

mod registration_fingerprints;
pub use registration_fingerprints::*;

mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
