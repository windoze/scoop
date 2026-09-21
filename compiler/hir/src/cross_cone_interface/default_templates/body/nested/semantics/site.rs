use scoop_wire::{WireError, WireErrorKind, WirePath};

/// A descriptor occurrence scoped to one default template. Body ordinals count
/// nested descriptors in preorder, independently of expression/reference indices.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultNestedCallableSiteV1 {
    /// An isolated descriptor check does not prove its position in a body.
    Standalone,
    Body {
        ordinal: u64,
    },
}

impl DefaultNestedCallableSiteV1 {
    pub(super) fn advance(&mut self, path: &WirePath) -> Result<Self, WireError> {
        let current = *self;
        if let Self::Body { ordinal } = self {
            *ordinal = ordinal.checked_add(1).ok_or_else(|| {
                WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
            })?;
        }
        Ok(current)
    }
}
