//! Frozen declaration-side callable membership for body lookup.
//!
//! Signature validation builds the rejected set off to the side and publishes
//! it in one step.  Rejected declarations retain their arena identities so
//! their own bodies and unrelated declarations can still be diagnosed, but no
//! body lookup may probe one as a semantic overload candidate.

use std::collections::HashSet;

use scoop_hir as hir;

#[derive(Clone, Default)]
pub(crate) struct DeclarationSurface {
    state: DeclarationSurfaceState,
}

#[derive(Clone, Default)]
enum DeclarationSurfaceState {
    #[default]
    Collecting,
    Frozen {
        rejected_functions: HashSet<hir::FunctionId>,
    },
}

impl DeclarationSurface {
    pub(crate) fn freeze(&mut self, rejected_functions: HashSet<hir::FunctionId>) {
        assert!(
            matches!(self.state, DeclarationSurfaceState::Collecting),
            "the declaration surface is frozen exactly once"
        );
        self.state = DeclarationSurfaceState::Frozen { rejected_functions };
    }

    pub(crate) fn is_frozen(&self) -> bool {
        matches!(self.state, DeclarationSurfaceState::Frozen { .. })
    }

    pub(crate) fn rejects_function(&self, function: hir::FunctionId) -> bool {
        match &self.state {
            DeclarationSurfaceState::Collecting => false,
            DeclarationSurfaceState::Frozen { rejected_functions } => {
                rejected_functions.contains(&function)
            }
        }
    }
}
