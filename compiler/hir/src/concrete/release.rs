use super::*;

pub type ReleaseHookId = Idx<ReleaseHook>;

#[derive(Debug, Clone)]
pub struct ReleaseHook {
    pub origin: crate::DefinitionOrigin,
    pub owner: ClassId,
    pub materialization: CallableMaterialization,
    pub body: Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseHookTarget {
    Local(ReleaseHookId),
    /// The provider emits the parameter-free owner's body and registration.
    External {
        owner: ClassId,
    },
}
