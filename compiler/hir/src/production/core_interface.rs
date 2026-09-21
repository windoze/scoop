//! Atomic HIR production interface for ordinary and trusted core Cones.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{ConeIdentity, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CanonicalDirectPublicSurfaceV1, CoreCompilerProtocolSurfaceBuildError,
    CoreCompilerProtocolSurfaceV1, CoreCompilerProtocolSurfaceValidationError,
    CoreHirTypeCapabilityV1, CorePreludeSnapshotBuildError, CorePreludeSnapshotV1,
    CorePreludeSnapshotValidationError, CoreTypeDefinitionV1, CoreTypeTargetSurfaceBuildError,
    CoreTypeTargetSurfaceV1, CoreTypeTargetSurfaceValidationError,
    DecodedCanonicalDirectPublicSurfaceV1, DecodedCoreCompilerProtocolSurfaceV1,
    DecodedCorePreludeSnapshotV1, DecodedCoreTypeTargetSurfaceV1, DecodedHirOutputContractV1,
    DecodedRuntimeCoreCapabilityV1, DirectPublicSurfaceBuildError,
    DirectPublicSurfaceValidationError, HirOutputContractV1, HirOutputContractValidationError,
    RuntimeCoreCapabilityBuildError, RuntimeCoreCapabilityV1, RuntimeCoreCapabilityValidationError,
};
use crate::{
    CanonicalHirFoundation, ExportHir, ExportHirOutput, OdrFreeHirFoundation,
    ValidatedHirFoundation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreHirInterfaceV1 {
    prelude_snapshot: CorePreludeSnapshotV1,
    string_capability: RuntimeCoreCapabilityV1,
    type_targets: CoreTypeTargetSurfaceV1,
    compiler_protocols: CoreCompilerProtocolSurfaceV1,
}

/// One parameter-free source nominal whose complete runtime shape is a
/// mandatory strong-production obligation of the trusted core Cone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreShapeSupportRequirementV1 {
    source: PersistentTypeId,
    exact: PersistentExactTypeId,
}

impl CoreShapeSupportRequirementV1 {
    pub const fn source(self) -> PersistentTypeId {
        self.source
    }

    pub const fn exact(self) -> PersistentExactTypeId {
        self.exact
    }
}

/// Complete, deterministically ordered shape-support requirement set derived
/// from the validated core HIR interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreShapeSupportRequirementsV1 {
    roots: Vec<CoreShapeSupportRequirementV1>,
}

impl CoreShapeSupportRequirementsV1 {
    pub fn roots(&self) -> &[CoreShapeSupportRequirementV1] {
        &self.roots
    }
}

impl CoreHirInterfaceV1 {
    pub fn from_core_export(export: &ExportHir) -> Result<Self, CoreHirInterfaceBuildError> {
        let direct_surface = CanonicalDirectPublicSurfaceV1::from_export_hir(export)
            .map_err(CoreHirInterfaceBuildError::DirectSurface)?;
        Self::from_core_export_against(export, &direct_surface)
    }

    fn from_core_export_against(
        export: &ExportHir,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<Self, CoreHirInterfaceBuildError> {
        if export.cone != ConeIdentity::CORE {
            return Err(CoreHirInterfaceBuildError::NotCore(export.cone));
        }
        let crate::CoreProtocols::Defined(protocols) = &export.core_protocols else {
            return Err(CoreHirInterfaceBuildError::ImportedProtocolsInCore);
        };
        let interface = Self {
            prelude_snapshot: CorePreludeSnapshotV1::from_core_export(export, protocols)
                .map_err(CoreHirInterfaceBuildError::Prelude)?,
            string_capability: RuntimeCoreCapabilityV1::string_from_core_export(export, protocols)
                .map_err(CoreHirInterfaceBuildError::String)?,
            type_targets: CoreTypeTargetSurfaceV1::from_core_export(export)
                .map_err(CoreHirInterfaceBuildError::TypeTargets)?,
            compiler_protocols: CoreCompilerProtocolSurfaceV1::from_core_export(export, protocols)
                .map_err(CoreHirInterfaceBuildError::CompilerProtocols)?,
        };
        validate_relations(&interface, direct_surface)
            .map_err(CoreHirInterfaceBuildError::Relation)?;
        Ok(interface)
    }

    pub const fn prelude_snapshot(&self) -> &CorePreludeSnapshotV1 {
        &self.prelude_snapshot
    }

    pub const fn string_capability(&self) -> RuntimeCoreCapabilityV1 {
        self.string_capability
    }

    pub const fn type_targets(&self) -> &CoreTypeTargetSurfaceV1 {
        &self.type_targets
    }

    pub const fn compiler_protocols(&self) -> &CoreCompilerProtocolSurfaceV1 {
        &self.compiler_protocols
    }

    /// Projects the authoritative source/exact pairs that every later stage
    /// must materialize. Aliases never introduce a second nominal root.
    pub fn shape_support_requirements(&self) -> CoreShapeSupportRequirementsV1 {
        let mut roots = BTreeMap::new();
        for target in self.type_targets.targets() {
            let (
                CoreTypeDefinitionV1::Type(source),
                CoreHirTypeCapabilityV1::ParamFreeStrong(exact),
            ) = (target.definition(), target.capability())
            else {
                continue;
            };
            roots
                .entry(source)
                .or_insert(CoreShapeSupportRequirementV1 { source, exact });
        }
        CoreShapeSupportRequirementsV1 {
            roots: roots.into_values().collect(),
        }
    }

    /// Derives the complete core shape-support obligation set from the
    /// already validated public type surface. Type aliases do not create a
    /// second obligation for their nominal target.
    pub fn param_free_shape_support_sources(
        &self,
        foundation: &OdrFreeHirFoundation,
    ) -> Result<Vec<SourceDeclarationKey>, CoreShapeSupportSourceProjectionError> {
        let mut sources = BTreeMap::new();
        for requirement in self.shape_support_requirements().roots {
            let source_type = requirement.source();
            let (_, source) = foundation
                .as_canonical()
                .source_type_by_bytes(source_type.as_array())
                .ok_or(CoreShapeSupportSourceProjectionError::MissingSourceNominal(
                    source_type,
                ))?;
            sources.entry(source_type).or_insert_with(|| source.clone());
        }
        Ok(sources.into_values().collect())
    }
}

impl WireEncode for CoreHirInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.prelude_snapshot.encode(encoder)?;
        encoder.field(2)?;
        self.string_capability.encode(encoder)?;
        encoder.field(4)?;
        self.type_targets.encode(encoder)?;
        encoder.field(6)?;
        self.compiler_protocols.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreHirInterfaceV1 {
    prelude_snapshot: DecodedCorePreludeSnapshotV1,
    string_capability: DecodedRuntimeCoreCapabilityV1,
    type_targets: DecodedCoreTypeTargetSurfaceV1,
    compiler_protocols: DecodedCoreCompilerProtocolSurfaceV1,
}

impl DecodedCoreHirInterfaceV1 {
    fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<CoreHirInterfaceV1, CoreHirInterfaceValidationError> {
        let interface = CoreHirInterfaceV1 {
            prelude_snapshot: self
                .prelude_snapshot
                .validate_against(foundation)
                .map_err(CoreHirInterfaceValidationError::Prelude)?,
            string_capability: self
                .string_capability
                .validate_against(foundation)
                .map_err(CoreHirInterfaceValidationError::String)?,
            type_targets: self
                .type_targets
                .validate_against(foundation, direct_surface)
                .map_err(CoreHirInterfaceValidationError::TypeTargets)?,
            compiler_protocols: self
                .compiler_protocols
                .validate_against(foundation)
                .map_err(CoreHirInterfaceValidationError::CompilerProtocols)?,
        };
        validate_relations(&interface, direct_surface)
            .map_err(CoreHirInterfaceValidationError::Relation)?;
        Ok(interface)
    }
}

impl WireEncode for DecodedCoreHirInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.prelude_snapshot.encode(encoder)?;
        encoder.field(2)?;
        self.string_capability.encode(encoder)?;
        encoder.field(4)?;
        self.type_targets.encode(encoder)?;
        encoder.field(6)?;
        self.compiler_protocols.encode(encoder)
    }
}

impl WireDecode for DecodedCoreHirInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            prelude_snapshot: decoder.field(1, DecodedCorePreludeSnapshotV1::decode)?,
            string_capability: decoder.field(2, DecodedRuntimeCoreCapabilityV1::decode)?,
            type_targets: decoder.field(4, DecodedCoreTypeTargetSurfaceV1::decode)?,
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
                CoreHirInterfaceV1::from_core_export_against(module, &direct_public_surface)
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
                        .validate_against(foundation, &direct_public_surface)
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

fn validate_relations(
    interface: &CoreHirInterfaceV1,
    direct_surface: &CanonicalDirectPublicSurfaceV1,
) -> Result<(), CoreHirInterfaceRelationError> {
    if interface.prelude_snapshot.ordinary_bindings() != direct_surface {
        return Err(CoreHirInterfaceRelationError::PreludeSurfaceMismatch);
    }
    let string_source = interface.string_capability.source_type();
    let string_exact = interface.string_capability.exact_type();
    let matching = interface.type_targets.targets().iter().filter(|target| {
        target.definition() == CoreTypeDefinitionV1::Type(string_source)
            && target.capability() == CoreHirTypeCapabilityV1::ParamFreeStrong(string_exact)
    });
    if matching.count() != 1 {
        return Err(CoreHirInterfaceRelationError::StringTypeTargetMismatch);
    }
    if interface.compiler_protocols.string_source_type() != string_source {
        return Err(CoreHirInterfaceRelationError::ProtocolStringMismatch);
    }
    if interface.compiler_protocols.option_some() != interface.prelude_snapshot.option_some()
        || interface.compiler_protocols.option_some_payload()
            != interface.prelude_snapshot.option_some_payload()
        || interface.compiler_protocols.option_none() != interface.prelude_snapshot.option_none()
    {
        return Err(CoreHirInterfaceRelationError::ProtocolOptionMismatch);
    }
    Ok(())
}

#[derive(Debug)]
pub enum CoreHirInterfaceBuildError {
    NotCore(ConeIdentity),
    ImportedProtocolsInCore,
    DirectSurface(DirectPublicSurfaceBuildError),
    Prelude(CorePreludeSnapshotBuildError),
    String(RuntimeCoreCapabilityBuildError),
    TypeTargets(CoreTypeTargetSurfaceBuildError),
    CompilerProtocols(CoreCompilerProtocolSurfaceBuildError),
    Relation(CoreHirInterfaceRelationError),
}

impl fmt::Display for CoreHirInterfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build core HIR interface: {self:?}")
    }
}

impl std::error::Error for CoreHirInterfaceBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreShapeSupportSourceProjectionError {
    MissingSourceNominal(PersistentTypeId),
}

impl fmt::Display for CoreShapeSupportSourceProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot project core shape-support source obligations: {self:?}"
        )
    }
}

impl std::error::Error for CoreShapeSupportSourceProjectionError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreHirInterfaceValidationError {
    Prelude(CorePreludeSnapshotValidationError),
    String(RuntimeCoreCapabilityValidationError),
    TypeTargets(CoreTypeTargetSurfaceValidationError),
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
    PreludeSurfaceMismatch,
    StringTypeTargetMismatch,
    ProtocolStringMismatch,
    ProtocolOptionMismatch,
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
