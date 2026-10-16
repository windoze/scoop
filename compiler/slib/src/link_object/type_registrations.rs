//! Type registration slots and descriptor/layout fingerprints.

mod binding;
pub use binding::*;
pub(in crate::link_object) mod record;
mod versioned;
pub(in crate::link_object) use versioned::{
    LinkDescriptorReference, LinkDispatchCallableReference,
};
mod dependency_fingerprints;
pub use dependency_fingerprints::*;
mod registration_fingerprints;
pub use registration_fingerprints::*;

#[cfg(test)]
mod tests;
