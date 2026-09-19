use super::*;
use crate::CheckedProtectedDefaultReferenceBodyV1;

#[derive(Debug)]
pub(super) struct Replay<'t, 's> {
    pub template: &'t ProtectedDefaultTemplateV1,
    pub owner: CheckedProtectedDefaultOwnerProfileV1<'s>,
    pub body: CheckedProtectedDefaultReferenceBodyV1<'t>,
}

/// Source-profile and every actual reference access have been replayed. Full
/// template contract, operation typing, local flow and table closure are still
/// required. Neither branch is a selection or default-expansion capability.
#[derive(Debug)]
pub enum CheckedProtectedDefaultReferenceReplayV1<'t, 's> {
    ParamFree(CheckedParamFreeProtectedDefaultReferenceReplayV1<'t, 's>),
    GenericSourceMetadata(CheckedGenericProtectedDefaultReferenceReplayV1<'t, 's>),
}
#[derive(Debug)]
pub struct CheckedParamFreeProtectedDefaultReferenceReplayV1<'t, 's>(pub(super) Replay<'t, 's>);
#[derive(Debug)]
pub struct CheckedGenericProtectedDefaultReferenceReplayV1<'t, 's>(pub(super) Replay<'t, 's>);

impl<'t, 's> CheckedProtectedDefaultReferenceReplayV1<'t, 's> {
    pub const fn template(&self) -> &'t ProtectedDefaultTemplateV1 {
        self.replay().template
    }
    pub const fn owner(&self) -> CheckedProtectedDefaultOwnerProfileV1<'s> {
        self.replay().owner
    }
    pub const fn body(&self) -> &CheckedProtectedDefaultReferenceBodyV1<'t> {
        &self.replay().body
    }
    const fn replay(&self) -> &Replay<'t, 's> {
        match self {
            Self::ParamFree(replay) => &replay.0,
            Self::GenericSourceMetadata(replay) => &replay.0,
        }
    }
}
