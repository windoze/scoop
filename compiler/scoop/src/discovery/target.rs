use std::borrow::Cow;
use std::path::Path;

use scoop_lir::{TargetProfileId, ValidatedLirTargetSelection};
use scoop_protocol::TargetSelectionRequestV1;
use scoop_toolchain::{ResolvedTargetProfile, ToolchainError};

/// Graph discovery needs a target contract; native production additionally
/// needs the selected tool invocation.
#[derive(Clone, Debug)]
pub enum GraphTarget {
    Artifact(Box<ResolvedTargetProfile>),
    Frontend {
        selection: ValidatedLirTargetSelection,
        request: TargetSelectionRequestV1,
    },
}

impl GraphTarget {
    pub const fn lir_target_selection(&self) -> ValidatedLirTargetSelection {
        match self {
            Self::Artifact(target) => target.lir_target_selection(),
            Self::Frontend { selection, .. } => *selection,
        }
    }

    pub const fn id(&self) -> TargetProfileId {
        self.lir_target_selection().target().id()
    }

    pub const fn canonical_triple(&self) -> &'static str {
        self.lir_target_selection()
            .target()
            .contract()
            .canonical_triple()
    }

    pub(crate) fn production(
        &self,
        cache: &Path,
    ) -> Result<Cow<'_, ResolvedTargetProfile>, ToolchainError> {
        match self {
            Self::Artifact(target) => Ok(Cow::Borrowed(target)),
            Self::Frontend { request, .. } => {
                ResolvedTargetProfile::resolve_request_cached(request, cache).map(Cow::Owned)
            }
        }
    }
}
