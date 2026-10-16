//! Runtime image data, fingerprint encoding and digest patching.

mod fingerprint;
pub use fingerprint::*;
mod finalization;
pub use finalization::*;
mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::empty_image_object_for_link_decode_test;
