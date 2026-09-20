use super::{DefaultSourceAccessBuildError, DefaultSourceAccessDomainV1};
use scoop_identity::CallableTemplateOrigin;

mod wire;
pub use wire::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionalDefaultSourceSlotDomainV1 {
    Absent,
    Present(DefaultSourceAccessDomainV1),
}

/// The provider-side source witness. An inherited publication must separately
/// replay the publishing owner's complete direct and root-slot requirements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultSourceAccessWitnessV1 {
    owner: CallableTemplateOrigin,
    direct: DefaultSourceAccessDomainV1,
    slot: OptionalDefaultSourceSlotDomainV1,
    target: DefaultSourceAccessDomainV1,
}
impl DefaultSourceAccessWitnessV1 {
    pub fn try_new(
        owner: CallableTemplateOrigin,
        direct: DefaultSourceAccessDomainV1,
        slot: OptionalDefaultSourceSlotDomainV1,
        target: DefaultSourceAccessDomainV1,
    ) -> Result<Self, DefaultSourceAccessBuildError> {
        if matches!(owner, CallableTemplateOrigin::Accessor(_)) {
            return Err(DefaultSourceAccessBuildError::AccessorOwner);
        }
        if !matches!(
            owner,
            CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_)
        ) && matches!(slot, OptionalDefaultSourceSlotDomainV1::Present(_))
        {
            return Err(DefaultSourceAccessBuildError::SlotForConstructor);
        }
        Ok(Self {
            owner,
            direct,
            slot,
            target,
        })
    }
    pub const fn owner(&self) -> CallableTemplateOrigin {
        self.owner
    }
    pub const fn direct_call_domain(&self) -> &DefaultSourceAccessDomainV1 {
        &self.direct
    }
    pub const fn slot_call_domain(&self) -> &OptionalDefaultSourceSlotDomainV1 {
        &self.slot
    }
    pub const fn target_domain(&self) -> &DefaultSourceAccessDomainV1 {
        &self.target
    }
}
