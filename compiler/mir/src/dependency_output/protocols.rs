//! Protocol selections retain their concrete ownership type across MIR lowering.

use crate::{SelectedImportedMirSet, StrongImportedCoreInput};

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::CurrentMirProtocolDeclarations {}
    impl Sealed for super::SelectedImportedMirSet<'_> {}
}

/// Compiler protocol declarations belong to the current HIR graph.
#[derive(Clone, Copy, Debug, Default)]
pub struct CurrentMirProtocolDeclarations;

/// Closed protocol input family shared by MIR lowering and strong sealing.
///
/// The output keeps the concrete input type, so callers retain either the
/// actual imported selection or the local declaration marker without an
/// optional sidecar or an artificial empty imported artifact.
pub trait MirProtocolSelection: sealed::Sealed {
    fn as_strong_input(&self) -> StrongImportedCoreInput<'_>;
}

impl MirProtocolSelection for CurrentMirProtocolDeclarations {
    fn as_strong_input(&self) -> StrongImportedCoreInput<'_> {
        StrongImportedCoreInput::Unused
    }
}

impl MirProtocolSelection for SelectedImportedMirSet<'_> {
    fn as_strong_input(&self) -> StrongImportedCoreInput<'_> {
        StrongImportedCoreInput::Selected(self)
    }
}
