//! HIR output contracts and locally defined compiler protocol roles.

use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CanonicalDirectPublicSurfaceV1, CoreCompilerProtocolSurfaceBuildError,
    CoreCompilerProtocolSurfaceV1, CoreCompilerProtocolSurfaceValidationError,
    DecodedCanonicalDirectPublicSurfaceV1, DecodedCoreCompilerProtocolSurfaceV1,
    DecodedHirOutputContractV1, DirectPublicSurfaceBuildError, DirectPublicSurfaceValidationError,
    HirOutputContractV1, HirOutputContractValidationError,
};
use crate::{CanonicalHirFoundation, ExportHirOutput, ValidatedHirFoundation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreBootstrapInterfaceSectionV1 {
    compiler_protocols: Option<Box<CoreCompilerProtocolSurfaceV1>>,
    output_contract: HirOutputContractV1,
    direct_public_surface: CanonicalDirectPublicSurfaceV1,
}

impl CoreBootstrapInterfaceSectionV1 {
    pub fn from_export(export: &ExportHirOutput) -> Result<Self, CoreBootstrapInterfaceBuildError> {
        let module = export.module();
        let direct_public_surface = CanonicalDirectPublicSurfaceV1::from_export_hir(module)
            .map_err(CoreBootstrapInterfaceBuildError::DirectSurface)?;
        let output_contract = HirOutputContractV1::from_output_kind(export.output_kind());
        let compiler_protocols = match &module.core_protocols {
            crate::CoreProtocols::Defined(protocols) => Some(Box::new(
                CoreCompilerProtocolSurfaceV1::from_export(module, protocols)
                    .map_err(CoreBootstrapInterfaceBuildError::ProtocolDefinitions)?,
            )),
            crate::CoreProtocols::Imported(_) => None,
        };
        Ok(Self {
            compiler_protocols,
            output_contract,
            direct_public_surface,
        })
    }

    pub fn compiler_protocols(&self) -> Option<&CoreCompilerProtocolSurfaceV1> {
        self.compiler_protocols.as_deref()
    }

    pub const fn output_contract(&self) -> &HirOutputContractV1 {
        &self.output_contract
    }

    pub const fn direct_public_surface(&self) -> &CanonicalDirectPublicSurfaceV1 {
        &self.direct_public_surface
    }
}

impl WireEncode for CoreBootstrapInterfaceSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(2)?;
        self.output_contract.encode(encoder)?;
        encoder.field(3)?;
        self.direct_public_surface.encode(encoder)?;
        encoder.field(5)?;
        encode_definitions(self.compiler_protocols.as_deref(), encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreBootstrapInterfaceSectionV1 {
    compiler_protocols: Option<Box<DecodedCoreCompilerProtocolSurfaceV1>>,
    output_contract: DecodedHirOutputContractV1,
    direct_public_surface: DecodedCanonicalDirectPublicSurfaceV1,
}

impl DecodedCoreBootstrapInterfaceSectionV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
    ) -> Result<CoreBootstrapInterfaceSectionV1, CoreBootstrapInterfaceValidationError> {
        self.validate_against(foundation.artifact(), foundation.canonical())
    }

    pub fn validate_against(
        self,
        artifact: ConeIdentity,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreBootstrapInterfaceSectionV1, CoreBootstrapInterfaceValidationError> {
        let direct_public_surface = self
            .direct_public_surface
            .validate_against(foundation)
            .map_err(CoreBootstrapInterfaceValidationError::DirectSurface)?;
        let output_contract = self
            .output_contract
            .validate_against(artifact, foundation)
            .map_err(CoreBootstrapInterfaceValidationError::OutputContract)?;
        let compiler_protocols = self
            .compiler_protocols
            .map(|definitions| definitions.validate_against(foundation).map(Box::new))
            .transpose()
            .map_err(CoreBootstrapInterfaceValidationError::ProtocolDefinitions)?;
        Ok(CoreBootstrapInterfaceSectionV1 {
            compiler_protocols,
            output_contract,
            direct_public_surface,
        })
    }
}

impl WireEncode for DecodedCoreBootstrapInterfaceSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(2)?;
        self.output_contract.encode(encoder)?;
        encoder.field(3)?;
        self.direct_public_surface.encode(encoder)?;
        encoder.field(5)?;
        encode_definitions(self.compiler_protocols.as_deref(), encoder)
    }
}

impl WireDecode for DecodedCoreBootstrapInterfaceSectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            output_contract: decoder.field(2, DecodedHirOutputContractV1::decode)?,
            direct_public_surface: decoder
                .field(3, DecodedCanonicalDirectPublicSurfaceV1::decode)?,
            compiler_protocols: decoder.field(5, |decoder| match decoder.array()? {
                0 => Ok(None),
                1 => decoder
                    .index(0, DecodedCoreCompilerProtocolSurfaceV1::decode)
                    .map(Box::new)
                    .map(Some),
                actual => Err(wire_error(
                    decoder,
                    WireErrorKind::InvalidLength {
                        expected: 1,
                        actual,
                    },
                )),
            })?,
        })
    }
}

#[derive(Debug)]
pub enum CoreBootstrapInterfaceBuildError {
    DirectSurface(DirectPublicSurfaceBuildError),
    ProtocolDefinitions(CoreCompilerProtocolSurfaceBuildError),
}

impl fmt::Display for CoreBootstrapInterfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build HIR production section: {self:?}")
    }
}

impl std::error::Error for CoreBootstrapInterfaceBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreBootstrapInterfaceValidationError {
    DirectSurface(DirectPublicSurfaceValidationError),
    OutputContract(HirOutputContractValidationError),
    ProtocolDefinitions(CoreCompilerProtocolSurfaceValidationError),
}

impl fmt::Display for CoreBootstrapInterfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid HIR production section: {self:?}")
    }
}

impl std::error::Error for CoreBootstrapInterfaceValidationError {}

fn encode_definitions(
    definitions: Option<&impl WireEncode>,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(u64::from(definitions.is_some()))?;
    if let Some(definitions) = definitions {
        definitions.encode(encoder)?;
    }
    Ok(())
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
use crate::OdrFreeHirFoundation;
