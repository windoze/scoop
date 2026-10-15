//! Classify actual external shape symbol uses against selected imports.

use super::CanonicalUndefinedRelocationUseV1;

mod classification;
mod error;

pub use classification::*;
pub use error::LayoutLinkClosureError;

/// A canonical actual use, indexed into the unchanged Compile import array.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalShapeUndefinedUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    import_index: u32,
}

impl ExternalShapeUndefinedUseV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }
    pub const fn import_index(&self) -> u32 {
        self.import_index
    }
}

#[cfg(test)]
pub(in crate::link_object) mod tests;
