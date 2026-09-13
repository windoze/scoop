use std::fmt;

use scoop_identity::{SourceDeclarationKey, ValidatedIdentityGraph};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use super::{
    DecodedParamFreeShapeSupportPlanSetV1, ParamFreeShapeSupportBuildError,
    ParamFreeShapeSupportPlanSetV1, ParamFreeShapeSupportValidationError,
};
use crate::{OdrFreeLirFoundation, StrongRegistrationIdentitySurfaceV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreShapeSupportPlanV1 {
    NotCore,
    Core(ParamFreeShapeSupportPlanSetV1),
}

impl CoreShapeSupportPlanV1 {
    pub fn new<'source>(
        sources: impl IntoIterator<Item = &'source SourceDeclarationKey>,
        foundation: &OdrFreeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
    ) -> Result<Self, CoreShapeSupportPlanBuildError> {
        let sources = sources.into_iter().collect::<Vec<_>>();
        if foundation.producer() == scoop_identity::ConeIdentity::CORE {
            ParamFreeShapeSupportPlanSetV1::from_core_sources(
                sources.iter().copied(),
                foundation,
                registrations,
            )
            .map(Self::Core)
            .map_err(CoreShapeSupportPlanBuildError::Core)
        } else if sources.is_empty() {
            Ok(Self::NotCore)
        } else {
            Err(CoreShapeSupportPlanBuildError::SourcesForNonCore {
                producer: foundation.producer(),
                count: sources.len(),
            })
        }
    }

    pub const fn core(&self) -> Option<&ParamFreeShapeSupportPlanSetV1> {
        match self {
            Self::NotCore => None,
            Self::Core(plan) => Some(plan),
        }
    }
}

impl WireEncode for CoreShapeSupportPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCore => encode_empty_sum(encoder, 1),
            Self::Core(plan) => encode_value_sum(encoder, 2, plan),
        }
    }
}

#[derive(Debug)]
pub enum DecodedCoreShapeSupportPlanV1 {
    NotCore,
    Core(DecodedParamFreeShapeSupportPlanSetV1),
}

impl DecodedCoreShapeSupportPlanV1 {
    pub fn validate<'source>(
        self,
        sources: impl IntoIterator<Item = &'source SourceDeclarationKey>,
        identities: &mut ValidatedIdentityGraph,
        foundation: &OdrFreeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
    ) -> Result<CoreShapeSupportPlanV1, CoreShapeSupportPlanValidationError> {
        let sources = sources.into_iter().collect::<Vec<_>>();
        let actual = encode(&self).map_err(CoreShapeSupportPlanValidationError::Encode)?;
        if let Self::Core(plan) = self {
            plan.validate(
                sources.iter().copied(),
                identities,
                foundation,
                registrations,
            )
            .map_err(CoreShapeSupportPlanValidationError::Core)?;
        }
        let expected =
            CoreShapeSupportPlanV1::new(sources.iter().copied(), foundation, registrations)
                .map_err(CoreShapeSupportPlanValidationError::Expected)?;
        let expected_bytes =
            encode(&expected).map_err(CoreShapeSupportPlanValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(CoreShapeSupportPlanValidationError::BranchMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedCoreShapeSupportPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCore => encode_empty_sum(encoder, 1),
            Self::Core(plan) => encode_value_sum(encoder, 2, plan),
        }
    }
}

impl WireDecode for DecodedCoreShapeSupportPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match (tag, fields) {
            (1, 1) => Ok(Self::NotCore),
            (2, 2) => decoder.field(1, |decoder| {
                DecodedParamFreeShapeSupportPlanSetV1::decode(decoder).map(Self::Core)
            }),
            _ if tag > 2 => Err(unknown_tag(decoder, tag)),
            _ => Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: if tag == 1 { 1 } else { 2 },
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Debug)]
pub enum CoreShapeSupportPlanBuildError {
    Core(ParamFreeShapeSupportBuildError),
    SourcesForNonCore {
        producer: scoop_identity::ConeIdentity,
        count: usize,
    },
}

impl fmt::Display for CoreShapeSupportPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid core shape-support branch: {self:?}")
    }
}

impl std::error::Error for CoreShapeSupportPlanBuildError {}

#[derive(Debug)]
pub enum CoreShapeSupportPlanValidationError {
    Encode(scoop_wire::cbor::EncodeError),
    Core(ParamFreeShapeSupportValidationError),
    Expected(CoreShapeSupportPlanBuildError),
    BranchMismatch,
}

impl fmt::Display for CoreShapeSupportPlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded core shape-support branch: {self:?}"
        )
    }
}

impl std::error::Error for CoreShapeSupportPlanValidationError {}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

#[cfg(test)]
mod tests;
