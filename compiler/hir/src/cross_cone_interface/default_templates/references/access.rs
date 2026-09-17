use scoop_identity::DecodedCallableTemplateOrigin;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{CallableDeclarationId, CallableDeclarationIdResolver};

/// Public call-domain shape carried by an exported default witness.
///
/// The general M21 access domain cannot cross the artifact boundary because
/// its restricted variants contain provider-local arena identities. Exported
/// source interfaces have already narrowed the successful cases to these two
/// universal public shapes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExportDefaultCallDomainV1 {
    DirectPublic,
    DirectAndPublicSlot,
}

impl WireEncode for ExportDefaultCallDomainV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::DirectPublic => 1,
            Self::DirectAndPublicSlot => 2,
        })
    }
}

impl WireDecode for ExportDefaultCallDomainV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::DirectPublic),
            2 => Ok(Self::DirectAndPublicSlot),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

/// Access-domain shape of every target admitted to an exported default.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExportDefaultTargetDomainV1 {
    Universal,
}

impl WireEncode for ExportDefaultTargetDomainV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

impl WireDecode for ExportDefaultTargetDomainV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Universal),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportDefaultAccessWitnessV1 {
    owner: CallableDeclarationId,
    call_domain: ExportDefaultCallDomainV1,
    target_domain: ExportDefaultTargetDomainV1,
}

impl ExportDefaultAccessWitnessV1 {
    pub const fn new(owner: CallableDeclarationId, call_domain: ExportDefaultCallDomainV1) -> Self {
        Self {
            owner,
            call_domain,
            target_domain: ExportDefaultTargetDomainV1::Universal,
        }
    }

    pub const fn owner(self) -> CallableDeclarationId {
        self.owner
    }

    pub const fn call_domain(self) -> ExportDefaultCallDomainV1 {
        self.call_domain
    }

    pub const fn target_domain(self) -> ExportDefaultTargetDomainV1 {
        self.target_domain
    }
}

impl WireEncode for ExportDefaultAccessWitnessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.call_domain.encode(encoder)?;
        encoder.field(3)?;
        self.target_domain.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedExportDefaultAccessWitnessV1 {
    owner: DecodedCallableTemplateOrigin,
    call_domain: ExportDefaultCallDomainV1,
    target_domain: ExportDefaultTargetDomainV1,
}

impl DecodedExportDefaultAccessWitnessV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<ExportDefaultAccessWitnessV1, E>
    where
        R: CallableDeclarationIdResolver<E>,
    {
        self.owner
            .resolve(resolver)
            .map(|owner| ExportDefaultAccessWitnessV1 {
                owner,
                call_domain: self.call_domain,
                target_domain: self.target_domain,
            })
    }
}

impl WireEncode for DecodedExportDefaultAccessWitnessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.call_domain.encode(encoder)?;
        encoder.field(3)?;
        self.target_domain.encode(encoder)
    }
}

impl WireDecode for DecodedExportDefaultAccessWitnessV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            call_domain: decoder.field(2, ExportDefaultCallDomainV1::decode)?,
            target_domain: decoder.field(3, ExportDefaultTargetDomainV1::decode)?,
        })
    }
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}
