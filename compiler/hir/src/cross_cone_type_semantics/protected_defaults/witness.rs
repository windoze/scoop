use super::CanonicalProtectedDefaultSlotCallDomainsV1;
use crate::PersistentLookupDomainV1;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{Encoder, WireEncode};

mod decode;
mod validation;
pub use decode::*;
pub use validation::*;

/// Claimed default access data. Validation separately joins source ownership,
/// all root slots, target access, and the complete typed body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedDefaultAccessWitnessV1(Witness);
#[derive(Clone, Debug, Eq, PartialEq)]
enum Witness {
    ParamFree(ParamFreeProtectedDefaultAccessWitnessV1),
    GenericSourceMetadata { owner: CallableTemplateOrigin },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeProtectedDefaultAccessWitnessV1 {
    owner: CallableTemplateOrigin,
    direct_call_domain: PersistentLookupDomainV1,
    slot_call_domains: CanonicalProtectedDefaultSlotCallDomainsV1,
    target_domain: PersistentLookupDomainV1,
}
impl ProtectedDefaultAccessWitnessV1 {
    pub fn param_free(
        owner: CallableTemplateOrigin,
        direct_call_domain: PersistentLookupDomainV1,
        slot_call_domains: CanonicalProtectedDefaultSlotCallDomainsV1,
        target_domain: PersistentLookupDomainV1,
    ) -> Result<Self, ProtectedDefaultAccessWitnessBuildError> {
        validate_owner(owner)?;
        if !matches!(owner, CallableTemplateOrigin::Function(_))
            && !slot_call_domains.records().is_empty()
        {
            return Err(ProtectedDefaultAccessWitnessBuildError::SlotsForNonDispatchOwner);
        }
        Ok(Self(Witness::ParamFree(
            ParamFreeProtectedDefaultAccessWitnessV1 {
                owner,
                direct_call_domain,
                slot_call_domains,
                target_domain,
            },
        )))
    }
    pub fn generic_source_metadata(
        owner: CallableTemplateOrigin,
    ) -> Result<Self, ProtectedDefaultAccessWitnessBuildError> {
        validate_owner(owner)?;
        Ok(Self(Witness::GenericSourceMetadata { owner }))
    }
    pub const fn owner(&self) -> CallableTemplateOrigin {
        match &self.0 {
            Witness::ParamFree(value) => value.owner,
            Witness::GenericSourceMetadata { owner } => *owner,
        }
    }
    pub const fn view(&self) -> ProtectedDefaultAccessWitnessViewV1<'_> {
        match &self.0 {
            Witness::ParamFree(value) => ProtectedDefaultAccessWitnessViewV1::ParamFree(value),
            Witness::GenericSourceMetadata { owner } => {
                ProtectedDefaultAccessWitnessViewV1::GenericSourceMetadata { owner: *owner }
            }
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum ProtectedDefaultAccessWitnessViewV1<'a> {
    ParamFree(&'a ParamFreeProtectedDefaultAccessWitnessV1),
    GenericSourceMetadata { owner: CallableTemplateOrigin },
}
impl ParamFreeProtectedDefaultAccessWitnessV1 {
    pub const fn owner(&self) -> CallableTemplateOrigin {
        self.owner
    }
    pub const fn direct_call_domain(&self) -> &PersistentLookupDomainV1 {
        &self.direct_call_domain
    }
    pub const fn slot_call_domains(&self) -> &CanonicalProtectedDefaultSlotCallDomainsV1 {
        &self.slot_call_domains
    }
    pub const fn target_domain(&self) -> &PersistentLookupDomainV1 {
        &self.target_domain
    }
}
fn validate_owner(
    owner: CallableTemplateOrigin,
) -> Result<(), ProtectedDefaultAccessWitnessBuildError> {
    if matches!(owner, CallableTemplateOrigin::Accessor(_)) {
        Err(ProtectedDefaultAccessWitnessBuildError::AccessorOwner)
    } else {
        Ok(())
    }
}
impl WireEncode for ProtectedDefaultAccessWitnessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let fields = if matches!(self.0, Witness::ParamFree(_)) {
            5
        } else {
            2
        };
        encoder.map(fields)?;
        encoder.field(0)?;
        encoder.unsigned(if fields == 5 { 1 } else { 2 })?;
        encoder.field(1)?;
        self.owner().encode(encoder)?;
        if let Witness::ParamFree(value) = &self.0 {
            encoder.field(2)?;
            value.direct_call_domain.encode(encoder)?;
            encoder.field(3)?;
            value.slot_call_domains.encode(encoder)?;
            encoder.field(4)?;
            value.target_domain.encode(encoder)?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedDefaultAccessWitnessBuildError {
    AccessorOwner,
    SlotsForNonDispatchOwner,
}
impl std::fmt::Display for ProtectedDefaultAccessWitnessBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::AccessorOwner => "accessors cannot own default templates",
            Self::SlotsForNonDispatchOwner => {
                "this default owner cannot have dispatch slot domains"
            }
        })
    }
}
impl std::error::Error for ProtectedDefaultAccessWitnessBuildError {}

#[cfg(test)]
mod tests;
