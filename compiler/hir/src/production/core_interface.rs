//! HIR output contracts and locally defined compiler protocol roles.

use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CanonicalDirectPublicSurfaceV1, CoreCompilerProtocolSurfaceBuildError,
    CoreCompilerProtocolSurfaceV1, CoreCompilerProtocolSurfaceValidationError,
    DecodedCanonicalDirectPublicSurfaceV1, DecodedCoreCompilerProtocolSurfaceV1,
    DecodedHirOutputContractV1, DecodedRuntimeCoreCapabilityV1, DirectPublicSurfaceBuildError,
    DirectPublicSurfaceValidationError, HirOutputContractV1, HirOutputContractValidationError,
    RuntimeCoreCapabilityBuildError, RuntimeCoreCapabilityV1, RuntimeCoreCapabilityValidationError,
};
use crate::{
    CanonicalHirFoundation, ExportHir, ExportHirOutput, OdrFreeHirFoundation,
    ValidatedHirFoundation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerProtocolDefinitionsV1 {
    string_capability: RuntimeCoreCapabilityV1,
    compiler_protocols: CoreCompilerProtocolSurfaceV1,
}

impl CompilerProtocolDefinitionsV1 {
    pub fn from_export(export: &ExportHir) -> Result<Self, CompilerProtocolDefinitionsBuildError> {
        let crate::CoreProtocols::Defined(protocols) = &export.core_protocols else {
            return Err(CompilerProtocolDefinitionsBuildError::NoLocalDefinitions);
        };
        let interface = Self {
            string_capability: RuntimeCoreCapabilityV1::string_from_export(export, protocols)
                .map_err(CompilerProtocolDefinitionsBuildError::String)?,
            compiler_protocols: CoreCompilerProtocolSurfaceV1::from_core_export(export, protocols)
                .map_err(CompilerProtocolDefinitionsBuildError::CompilerProtocols)?,
        };
        validate_relations(&interface).map_err(CompilerProtocolDefinitionsBuildError::Relation)?;
        Ok(interface)
    }

    pub const fn string_capability(&self) -> RuntimeCoreCapabilityV1 {
        self.string_capability
    }

    pub const fn compiler_protocols(&self) -> &CoreCompilerProtocolSurfaceV1 {
        &self.compiler_protocols
    }
}

impl WireEncode for CompilerProtocolDefinitionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(2)?;
        self.string_capability.encode(encoder)?;
        encoder.field(6)?;
        self.compiler_protocols.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCompilerProtocolDefinitionsV1 {
    string_capability: DecodedRuntimeCoreCapabilityV1,
    compiler_protocols: DecodedCoreCompilerProtocolSurfaceV1,
}

impl DecodedCompilerProtocolDefinitionsV1 {
    fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CompilerProtocolDefinitionsV1, CompilerProtocolDefinitionsValidationError> {
        let interface = CompilerProtocolDefinitionsV1 {
            string_capability: self
                .string_capability
                .validate_against(foundation)
                .map_err(CompilerProtocolDefinitionsValidationError::String)?,
            compiler_protocols: self
                .compiler_protocols
                .validate_against(foundation)
                .map_err(CompilerProtocolDefinitionsValidationError::CompilerProtocols)?,
        };
        validate_relations(&interface)
            .map_err(CompilerProtocolDefinitionsValidationError::Relation)?;
        Ok(interface)
    }
}

impl WireEncode for DecodedCompilerProtocolDefinitionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(2)?;
        self.string_capability.encode(encoder)?;
        encoder.field(6)?;
        self.compiler_protocols.encode(encoder)
    }
}

impl WireDecode for DecodedCompilerProtocolDefinitionsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            string_capability: decoder.field(2, DecodedRuntimeCoreCapabilityV1::decode)?,
            compiler_protocols: decoder.field(6, DecodedCoreCompilerProtocolSurfaceV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreBootstrapInterfaceSectionV1 {
    protocol_definitions: Option<Box<CompilerProtocolDefinitionsV1>>,
    output_contract: HirOutputContractV1,
    direct_public_surface: CanonicalDirectPublicSurfaceV1,
}

impl CoreBootstrapInterfaceSectionV1 {
    pub fn from_export(export: &ExportHirOutput) -> Result<Self, CoreBootstrapInterfaceBuildError> {
        let module = export.module();
        let direct_public_surface = CanonicalDirectPublicSurfaceV1::from_export_hir(module)
            .map_err(CoreBootstrapInterfaceBuildError::DirectSurface)?;
        let output_contract = HirOutputContractV1::from_output_kind(export.output_kind());
        let protocol_definitions = match &module.core_protocols {
            crate::CoreProtocols::Defined(_) => Some(Box::new(
                CompilerProtocolDefinitionsV1::from_export(module)
                    .map_err(CoreBootstrapInterfaceBuildError::ProtocolDefinitions)?,
            )),
            crate::CoreProtocols::Imported(_) => None,
        };
        Ok(Self {
            protocol_definitions,
            output_contract,
            direct_public_surface,
        })
    }

    pub fn compiler_protocol_definitions(&self) -> Option<&CompilerProtocolDefinitionsV1> {
        self.protocol_definitions.as_deref()
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
        encoder.field(4)?;
        encode_definitions(self.protocol_definitions.as_deref(), encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreBootstrapInterfaceSectionV1 {
    protocol_definitions: Option<Box<DecodedCompilerProtocolDefinitionsV1>>,
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

    pub fn validate_against_strong_foundation(
        self,
        artifact: ConeIdentity,
        foundation: &OdrFreeHirFoundation,
    ) -> Result<CoreBootstrapInterfaceSectionV1, CoreBootstrapInterfaceValidationError> {
        self.validate_against(artifact, foundation.as_canonical())
    }

    fn validate_against(
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
        let protocol_definitions = self
            .protocol_definitions
            .map(|definitions| definitions.validate_against(foundation).map(Box::new))
            .transpose()
            .map_err(CoreBootstrapInterfaceValidationError::ProtocolDefinitions)?;
        Ok(CoreBootstrapInterfaceSectionV1 {
            protocol_definitions,
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
        encoder.field(4)?;
        encode_definitions(self.protocol_definitions.as_deref(), encoder)
    }
}

impl WireDecode for DecodedCoreBootstrapInterfaceSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            output_contract: decoder.field(2, DecodedHirOutputContractV1::decode)?,
            direct_public_surface: decoder
                .field(3, DecodedCanonicalDirectPublicSurfaceV1::decode)?,
            protocol_definitions: decoder.field(4, |decoder| match decoder.array()? {
                0 => Ok(None),
                1 => decoder
                    .index(0, DecodedCompilerProtocolDefinitionsV1::decode)
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

fn validate_relations(
    interface: &CompilerProtocolDefinitionsV1,
) -> Result<(), CompilerProtocolDefinitionsRelationError> {
    let string_source = interface.string_capability.source_type();
    if interface.compiler_protocols.string_source_type() != string_source {
        return Err(CompilerProtocolDefinitionsRelationError::ProtocolStringMismatch);
    }
    Ok(())
}

#[derive(Debug)]
pub enum CompilerProtocolDefinitionsBuildError {
    NoLocalDefinitions,
    String(RuntimeCoreCapabilityBuildError),
    CompilerProtocols(CoreCompilerProtocolSurfaceBuildError),
    Relation(CompilerProtocolDefinitionsRelationError),
}

impl fmt::Display for CompilerProtocolDefinitionsBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot build compiler protocol definitions: {self:?}"
        )
    }
}

impl std::error::Error for CompilerProtocolDefinitionsBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CompilerProtocolDefinitionsValidationError {
    String(RuntimeCoreCapabilityValidationError),
    CompilerProtocols(CoreCompilerProtocolSurfaceValidationError),
    Relation(CompilerProtocolDefinitionsRelationError),
}

impl fmt::Display for CompilerProtocolDefinitionsValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid compiler protocol definitions: {self:?}")
    }
}

impl std::error::Error for CompilerProtocolDefinitionsValidationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerProtocolDefinitionsRelationError {
    ProtocolStringMismatch,
}

impl fmt::Display for CompilerProtocolDefinitionsRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid compiler protocol relation: {self:?}")
    }
}

impl std::error::Error for CompilerProtocolDefinitionsRelationError {}

#[derive(Debug)]
pub enum CoreBootstrapInterfaceBuildError {
    DirectSurface(DirectPublicSurfaceBuildError),
    ProtocolDefinitions(CompilerProtocolDefinitionsBuildError),
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
    ProtocolDefinitions(CompilerProtocolDefinitionsValidationError),
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

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
