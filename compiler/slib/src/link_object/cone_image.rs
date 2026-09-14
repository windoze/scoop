//! Exact provisional-object verification for the per-Cone runtime image.

mod error;
pub use error::*;

mod fingerprint;
mod physical;
mod record;
pub use fingerprint::*;
mod finalization;
pub use finalization::*;
mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::empty_image_object_for_link_decode_test;
