//! Runtime image data and artifact content fingerprint encoding.

mod fingerprint;
pub use fingerprint::*;
mod verification;
pub use verification::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::empty_image_object_for_link_decode_test;
