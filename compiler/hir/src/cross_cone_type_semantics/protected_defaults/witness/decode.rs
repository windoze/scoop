use super::super::{
    DecodedCanonicalProtectedDefaultSlotCallDomainsV1,
    ProtectedDefaultSlotCallDomainResolutionError, ProtectedDefaultSlotCallDomainResolver,
};
use super::*;
use crate::cross_cone_type_semantics::wire;
use crate::{
    CallableDeclarationIdResolver, DecodedPersistentAccessDomainV1, PersistentAccessResolutionError,
};
use scoop_identity::{CallableTemplateOrigin, DecodedCallableTemplateOrigin};
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath,
};

pub trait ProtectedDefaultAccessWitnessResolver<E>:
    ProtectedDefaultSlotCallDomainResolver<E> + CallableDeclarationIdResolver<E>
{
}
impl<R, E> ProtectedDefaultAccessWitnessResolver<E> for R where
    R: ProtectedDefaultSlotCallDomainResolver<E> + CallableDeclarationIdResolver<E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedProtectedDefaultAccessWitnessV1 {
    ParamFree {
        owner: DecodedCallableTemplateOrigin,
        direct_call_domain: DecodedPersistentAccessDomainV1,
        slot_call_domains: DecodedCanonicalProtectedDefaultSlotCallDomainsV1,
        target_domain: DecodedPersistentAccessDomainV1,
    },
    GenericSourceMetadata {
        owner: DecodedCallableTemplateOrigin,
    },
}
impl DecodedProtectedDefaultAccessWitnessV1 {
    pub fn resolve<R: ProtectedDefaultAccessWitnessResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedDefaultAccessWitnessV1, ProtectedDefaultAccessWitnessResolutionError<E>>
    {
        use ProtectedDefaultAccessWitnessResolutionError as Error;
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .charge_work(1, &WirePath::root())
            .map_err(Error::Resource)?;
        match self {
            Self::ParamFree {
                owner,
                direct_call_domain,
                slot_call_domains,
                target_domain,
            } => {
                let owner = resolve_owner(owner, resolver).map_err(Error::Foundation)?;
                let direct = direct_call_domain
                    .resolve_metered(resolver, meter)
                    .map_err(Error::Domain)?;
                let slots = slot_call_domains
                    .resolve(resolver, meter)
                    .map_err(Error::Slots)?;
                let target = target_domain
                    .resolve_metered(resolver, meter)
                    .map_err(Error::Domain)?;
                ProtectedDefaultAccessWitnessV1::param_free(
                    owner,
                    PersistentLookupDomainV1::new(direct),
                    slots,
                    PersistentLookupDomainV1::new(target),
                )
                .map_err(Error::Build)
            }
            Self::GenericSourceMetadata { owner } => {
                ProtectedDefaultAccessWitnessV1::generic_source_metadata(
                    resolve_owner(owner, resolver).map_err(Error::Foundation)?,
                )
                .map_err(Error::Build)
            }
        }
    }
}
fn resolve_owner<R: CallableDeclarationIdResolver<E>, E>(
    owner: DecodedCallableTemplateOrigin,
    resolver: &mut R,
) -> Result<CallableTemplateOrigin, E> {
    owner.resolve(resolver)
}
impl WireEncode for DecodedProtectedDefaultAccessWitnessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFree {
                owner,
                direct_call_domain,
                slot_call_domains,
                target_domain,
            } => {
                wire::tag(encoder, 5, 1)?;
                encoder.field(1)?;
                owner.encode(encoder)?;
                encoder.field(2)?;
                direct_call_domain.encode(encoder)?;
                encoder.field(3)?;
                slot_call_domains.encode(encoder)?;
                encoder.field(4)?;
                target_domain.encode(encoder)
            }
            Self::GenericSourceMetadata { owner } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                owner.encode(encoder)
            }
        }
    }
}
impl WireDecode for DecodedProtectedDefaultAccessWitnessV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 5)?;
                Ok(Self::ParamFree {
                    owner: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
                    direct_call_domain: decoder
                        .field(2, DecodedPersistentAccessDomainV1::decode)?,
                    slot_call_domains: decoder
                        .field(3, DecodedCanonicalProtectedDefaultSlotCallDomainsV1::decode)?,
                    target_domain: decoder.field(4, DecodedPersistentAccessDomainV1::decode)?,
                })
            }
            2 => {
                wire::expect_fields(decoder, fields, 2)?;
                Ok(Self::GenericSourceMetadata {
                    owner: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
                })
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
#[derive(Debug)]
pub enum ProtectedDefaultAccessWitnessResolutionError<E> {
    Resource(WireError),
    Foundation(E),
    Domain(PersistentAccessResolutionError<E>),
    Slots(ProtectedDefaultSlotCallDomainResolutionError<E>),
    Build(ProtectedDefaultAccessWitnessBuildError),
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultAccessWitnessResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Domain(error) => error.fmt(f),
            Self::Slots(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultAccessWitnessResolutionError<E>
{
}
