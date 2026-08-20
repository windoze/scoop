use crate::{ClassId, Function, Idx};

pub type ReleaseHookId = Idx<ReleaseHook>;

#[derive(Debug)]
pub struct ReleaseHook {
    pub owner: ClassId,
    pub materialization: scoop_identity::CallableMaterialization,
    /// Reuses the CFG storage, but never belongs to the ordinary function arena.
    pub code: Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReleasePolicy {
    #[default]
    None,
    SynchronousGcFree {
        hook: ReleaseHookTarget,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseHookTarget {
    Local(ReleaseHookId),
    External { owner: ClassId },
}
