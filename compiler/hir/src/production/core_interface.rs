//! Atomic HIR production interface for ordinary and trusted core Cones.

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
pub struct CoreHirInterfaceV1 {
    string_capability: RuntimeCoreCapabilityV1,
    compiler_protocols: CoreCompilerProtocolSurfaceV1,
}

impl CoreHirInterfaceV1 {
    pub fn from_core_export(export: &ExportHir) -> Result<Self, CoreHirInterfaceBuildError> {
        if export.cone != ConeIdentity::CORE {
            return Err(CoreHirInterfaceBuildError::NotCore(export.cone));
        }
        let crate::CoreProtocols::Defined(protocols) = &export.core_protocols else {
            return Err(CoreHirInterfaceBuildError::ImportedProtocolsInCore);
        };
        let interface = Self {
            string_capability: RuntimeCoreCapabilityV1::string_from_core_export(export, protocols)
                .map_err(CoreHirInterfaceBuildError::String)?,
            compiler_protocols: CoreCompilerProtocolSurfaceV1::from_core_export(export, protocols)
                .map_err(CoreHirInterfaceBuildError::CompilerProtocols)?,
        };
        validate_relations(&interface).map_err(CoreHirInterfaceBuildError::Relation)?;
        Ok(interface)
    }

    pub const fn string_capability(&self) -> RuntimeCoreCapabilityV1 {
        self.string_capability
    }

    pub const fn compiler_protocols(&self) -> &CoreCompilerProtocolSurfaceV1 {
        &self.compiler_protocols
    }
}

impl WireEncode for CoreHirInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(2)?;
        self.string_capability.encode(encoder)?;
        encoder.field(6)?;
        self.compiler_protocols.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreHirInterfaceV1 {
    string_capability: DecodedRuntimeCoreCapabilityV1,
    compiler_protocols: DecodedCoreCompilerProtocolSurfaceV1,
}

impl DecodedCoreHirInterfaceV1 {
    fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreHirInterfaceV1, CoreHirInterfaceValidationError> {
        let interface = CoreHirInterfaceV1 {
            string_capability: self
                .string_capability
                .validate_against(foundation)
                .map_err(CoreHirInterfaceValidationError::String)?,
            compiler_protocols: self
                .compiler_protocols
                .validate_against(foundation)
                .map_err(CoreHirInterfaceValidationError::CompilerProtocols)?,
        };
        validate_relations(&interface).map_err(CoreHirInterfaceValidationError::Relation)?;
        Ok(interface)
    }
}

impl WireEncode for DecodedCoreHirInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(2)?;
        self.string_capability.encode(encoder)?;
        encoder.field(6)?;
        self.compiler_protocols.encode(encoder)
    }
}

impl WireDecode for DecodedCoreHirInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            string_capability: decoder.field(2, DecodedRuntimeCoreCapabilityV1::decode)?,
            compiler_protocols: decoder.field(6, DecodedCoreCompilerProtocolSurfaceV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreHirInterfaceBranchV1 {
    NotCore,
    Core(Box<CoreHirInterfaceV1>),
}

impl WireEncode for CoreHirInterfaceBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCore => encode_empty_sum(encoder, 1),
            Self::Core(interface) => encode_value_sum(encoder, 2, interface.as_ref()),
        }
    }
}

#[derive(Debug)]
pub enum DecodedCoreHirInterfaceBranchV1 {
    NotCore,
    Core(Box<DecodedCoreHirInterfaceV1>),
}

impl WireEncode for DecodedCoreHirInterfaceBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCore => encode_empty_sum(encoder, 1),
            Self::Core(interface) => encode_value_sum(encoder, 2, interface.as_ref()),
        }
    }
}

impl WireDecode for DecodedCoreHirInterfaceBranchV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, fields, 1)?;
                Ok(Self::NotCore)
            }
            2 => {
                require_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedCoreHirInterfaceV1::decode)
                    .map(Box::new)
                    .map(Self::Core)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreBootstrapInterfaceSectionV1 {
    core_interface: CoreHirInterfaceBranchV1,
    output_contract: HirOutputContractV1,
    direct_public_surface: CanonicalDirectPublicSurfaceV1,
}

impl CoreBootstrapInterfaceSectionV1 {
    pub fn from_export(export: &ExportHirOutput) -> Result<Self, CoreBootstrapInterfaceBuildError> {
        let module = export.module();
        let direct_public_surface = CanonicalDirectPublicSurfaceV1::from_export_hir(module)
            .map_err(CoreBootstrapInterfaceBuildError::DirectSurface)?;
        let output_contract = HirOutputContractV1::from_output_kind(export.output_kind());
        let core_interface = if module.cone == ConeIdentity::CORE {
            if output_contract != HirOutputContractV1::Library {
                return Err(CoreBootstrapInterfaceBuildError::CoreMustBeLibrary);
            }
            CoreHirInterfaceBranchV1::Core(Box::new(
                CoreHirInterfaceV1::from_core_export(module)
                    .map_err(CoreBootstrapInterfaceBuildError::CoreInterface)?,
            ))
        } else {
            CoreHirInterfaceBranchV1::NotCore
        };
        Ok(Self {
            core_interface,
            output_contract,
            direct_public_surface,
        })
    }

    pub const fn core_interface(&self) -> &CoreHirInterfaceBranchV1 {
        &self.core_interface
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
        encoder.field(1)?;
        self.core_interface.encode(encoder)?;
        encoder.field(2)?;
        self.output_contract.encode(encoder)?;
        encoder.field(3)?;
        self.direct_public_surface.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreBootstrapInterfaceSectionV1 {
    core_interface: DecodedCoreHirInterfaceBranchV1,
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
        let core_interface = match (artifact == ConeIdentity::CORE, self.core_interface) {
            (false, DecodedCoreHirInterfaceBranchV1::NotCore) => CoreHirInterfaceBranchV1::NotCore,
            (false, DecodedCoreHirInterfaceBranchV1::Core(_)) => {
                return Err(
                    CoreBootstrapInterfaceValidationError::UnexpectedCoreInterface(artifact),
                );
            }
            (true, DecodedCoreHirInterfaceBranchV1::NotCore) => {
                return Err(CoreBootstrapInterfaceValidationError::MissingCoreInterface);
            }
            (true, DecodedCoreHirInterfaceBranchV1::Core(interface)) => {
                if output_contract != HirOutputContractV1::Library {
                    return Err(CoreBootstrapInterfaceValidationError::CoreMustBeLibrary);
                }
                CoreHirInterfaceBranchV1::Core(Box::new(
                    interface
                        .validate_against(foundation)
                        .map_err(CoreBootstrapInterfaceValidationError::CoreInterface)?,
                ))
            }
        };
        Ok(CoreBootstrapInterfaceSectionV1 {
            core_interface,
            output_contract,
            direct_public_surface,
        })
    }
}

impl WireEncode for DecodedCoreBootstrapInterfaceSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.core_interface.encode(encoder)?;
        encoder.field(2)?;
        self.output_contract.encode(encoder)?;
        encoder.field(3)?;
        self.direct_public_surface.encode(encoder)
    }
}

impl WireDecode for DecodedCoreBootstrapInterfaceSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            core_interface: decoder.field(1, DecodedCoreHirInterfaceBranchV1::decode)?,
            output_contract: decoder.field(2, DecodedHirOutputContractV1::decode)?,
            direct_public_surface: decoder
                .field(3, DecodedCanonicalDirectPublicSurfaceV1::decode)?,
        })
    }
}

fn validate_relations(interface: &CoreHirInterfaceV1) -> Result<(), CoreHirInterfaceRelationError> {
    let string_source = interface.string_capability.source_type();
    if interface.compiler_protocols.string_source_type() != string_source {
        return Err(CoreHirInterfaceRelationError::ProtocolStringMismatch);
    }
    Ok(())
}

#[derive(Debug)]
pub enum CoreHirInterfaceBuildError {
    NotCore(ConeIdentity),
    ImportedProtocolsInCore,
    String(RuntimeCoreCapabilityBuildError),
    CompilerProtocols(CoreCompilerProtocolSurfaceBuildError),
    Relation(CoreHirInterfaceRelationError),
}

impl fmt::Display for CoreHirInterfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build core HIR interface: {self:?}")
    }
}

impl std::error::Error for CoreHirInterfaceBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreHirInterfaceValidationError {
    String(RuntimeCoreCapabilityValidationError),
    CompilerProtocols(CoreCompilerProtocolSurfaceValidationError),
    Relation(CoreHirInterfaceRelationError),
}

impl fmt::Display for CoreHirInterfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid core HIR interface: {self:?}")
    }
}

impl std::error::Error for CoreHirInterfaceValidationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreHirInterfaceRelationError {
    ProtocolStringMismatch,
}

impl fmt::Display for CoreHirInterfaceRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid core HIR constituent relation: {self:?}")
    }
}

impl std::error::Error for CoreHirInterfaceRelationError {}

#[derive(Debug)]
pub enum CoreBootstrapInterfaceBuildError {
    DirectSurface(DirectPublicSurfaceBuildError),
    CoreMustBeLibrary,
    CoreInterface(CoreHirInterfaceBuildError),
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
    UnexpectedCoreInterface(ConeIdentity),
    MissingCoreInterface,
    CoreMustBeLibrary,
    CoreInterface(CoreHirInterfaceValidationError),
}

impl fmt::Display for CoreBootstrapInterfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid HIR production section: {self:?}")
    }
}

impl std::error::Error for CoreBootstrapInterfaceValidationError {}

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

fn require_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
