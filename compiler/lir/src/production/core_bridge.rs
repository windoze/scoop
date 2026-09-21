//! Initialization-role publication using the shared callable ABI record.

use std::fmt;

use crate::{
    CallableAbiDecodeError, CallableAbiRecordV1, CallableAbiValidationError, ConeIdentity,
    DecodedCallableAbiRecordV1, OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1,
};
use scoop_identity::ValidatedIdentityGraph;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::core_lir_cycle_thrower_for_test;

/// Complete canonical Scoop ABI publication surface of the core Cone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreLirBridgeV1 {
    initialization_cycle_thrower: Box<CallableAbiRecordV1>,
}

impl CoreLirBridgeV1 {
    pub fn new(initialization_cycle_thrower: CallableAbiRecordV1) -> Self {
        Self {
            initialization_cycle_thrower: Box::new(initialization_cycle_thrower),
        }
    }

    pub fn initialization_cycle_thrower(&self) -> &CallableAbiRecordV1 {
        &self.initialization_cycle_thrower
    }

    fn validate_foundation(
        &self,
        foundation: &OdrFreeLirFoundation,
        definitions: &StrongObjectSymbolSurfaceV1,
    ) -> Result<(), CoreLirBridgeBuildError> {
        self.initialization_cycle_thrower
            .validate_against(foundation, definitions)
            .map_err(CoreLirBridgeBuildError::Definition)
    }
}

impl WireEncode for CoreLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(2)?;
        self.initialization_cycle_thrower.encode(encoder)
    }
}

/// Closed producer-kind branch. Only the core Cone may publish a core bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreLirBridgeBranchV1 {
    NotCore,
    Core(CoreLirBridgeV1),
}

impl CoreLirBridgeBranchV1 {
    pub fn validate_against(
        &self,
        foundation: &OdrFreeLirFoundation,
        definitions: &StrongObjectSymbolSurfaceV1,
    ) -> Result<(), CoreLirBridgeBuildError> {
        match (foundation.producer() == ConeIdentity::CORE, self) {
            (false, Self::NotCore) => Ok(()),
            (true, Self::Core(bridge)) => bridge.validate_foundation(foundation, definitions),
            _ => Err(CoreLirBridgeBuildError::ProducerBranchMismatch),
        }
    }

    pub const fn core(&self) -> Option<&CoreLirBridgeV1> {
        match self {
            Self::NotCore => None,
            Self::Core(bridge) => Some(bridge),
        }
    }
}

impl WireEncode for CoreLirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(match self {
            Self::NotCore => 1,
            Self::Core(_) => 2,
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::NotCore => 1,
            Self::Core(_) => 2,
        })?;
        if let Self::Core(bridge) = self {
            encoder.field(1)?;
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DecodedCoreLirBridgeV1 {
    initialization_cycle_thrower: Box<DecodedCallableAbiRecordV1>,
}

impl WireEncode for DecodedCoreLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(2)?;
        self.initialization_cycle_thrower.encode(encoder)
    }
}

impl WireDecode for DecodedCoreLirBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        Ok(Self {
            initialization_cycle_thrower: Box::new(
                decoder.field(2, DecodedCallableAbiRecordV1::decode)?,
            ),
        })
    }
}

#[derive(Debug)]
pub enum DecodedCoreLirBridgeBranchV1 {
    NotCore,
    Core(DecodedCoreLirBridgeV1),
}

impl DecodedCoreLirBridgeBranchV1 {
    pub fn validate(
        self,
        foundation: &OdrFreeLirFoundation,
        definitions: &StrongObjectSymbolSurfaceV1,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CoreLirBridgeBranchV1, CoreLirBridgeValidationError> {
        let branch = match self {
            Self::NotCore => CoreLirBridgeBranchV1::NotCore,
            Self::Core(decoded) => {
                let cycle = decoded
                    .initialization_cycle_thrower
                    .validate(foundation.producer(), identities)
                    .map_err(CoreLirBridgeValidationError::Callable)?;
                CoreLirBridgeBranchV1::Core(CoreLirBridgeV1::new(cycle))
            }
        };
        branch
            .validate_against(foundation, definitions)
            .map_err(CoreLirBridgeValidationError::Build)?;
        Ok(branch)
    }
}

impl WireEncode for DecodedCoreLirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(match self {
            Self::NotCore => 1,
            Self::Core(_) => 2,
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::NotCore => 1,
            Self::Core(_) => 2,
        })?;
        if let Self::Core(bridge) = self {
            encoder.field(1)?;
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCoreLirBridgeBranchV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(WireError::new(
                WireErrorKind::MissingField { field: 0 },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match (tag, fields) {
            (1, 1) => Ok(Self::NotCore),
            (2, 2) => decoder
                .field(1, DecodedCoreLirBridgeV1::decode)
                .map(Self::Core),
            (1 | 2, actual) => Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: if tag == 1 { 1 } else { 2 },
                    actual,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
            (tag, _) => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Debug)]
pub enum CoreLirBridgeBuildError {
    Definition(CallableAbiValidationError),
    ProducerBranchMismatch,
}

impl fmt::Display for CoreLirBridgeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build core LIR bridge: {self:?}")
    }
}

impl std::error::Error for CoreLirBridgeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Definition(error) => Some(error),
            Self::ProducerBranchMismatch => None,
        }
    }
}

#[derive(Debug)]
pub enum CoreLirBridgeValidationError {
    Callable(CallableAbiDecodeError),
    Build(CoreLirBridgeBuildError),
}

impl fmt::Display for CoreLirBridgeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid decoded core LIR bridge: {self:?}")
    }
}

impl std::error::Error for CoreLirBridgeValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Callable(error) => Some(error),
            Self::Build(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests;
