use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::StageDumpPolicyV1;
use crate::dump::DecodedStageDumpPolicyV1;
use crate::path::DecodedHostPathCarrier;
use crate::{HostPathCarrier, PROTOCOL_VERSION, ProtocolValidationError, RequestCorrelationId};

mod target;
use target::DecodedTargetSelectionRequestV1;
pub use target::TargetSelectionRequestV1;

use crate::OptimizationMode;

const REQUEST_MAGIC: &[u8; 8] = b"SCOOPREQ";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurrentConeRequestV1 {
    ManifestRoot { root: HostPathCarrier },
    SingleFile { source: HostPathCarrier },
}

impl WireEncode for CurrentConeRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ManifestRoot { root } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                root.encode(encoder)
            }
            Self::SingleFile { source } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                source.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrustedCoreRequestV1 {
    ArtifactSlot { artifact: HostPathCarrier },
    Bootstrap,
}

impl WireEncode for TrustedCoreRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ArtifactSlot { artifact } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                artifact.encode(encoder)
            }
            Self::Bootstrap => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(2)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticOutputPolicyV1 {
    Human,
    Structured,
}

impl DiagnosticOutputPolicyV1 {
    fn tag(self) -> u64 {
        match self {
            Self::Human => 1,
            Self::Structured => 2,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, ProtocolValidationError> {
        match tag {
            1 => Ok(Self::Human),
            2 => Ok(Self::Structured),
            _ => Err(ProtocolValidationError::UnknownEnumTag {
                kind: "diagnostic output policy",
                tag,
            }),
        }
    }
}

impl WireEncode for DiagnosticOutputPolicyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.tag())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopcBuildRequestV1 {
    current: CurrentConeRequestV1,
    direct_slibs: Vec<HostPathCarrier>,
    support_slibs: Vec<HostPathCarrier>,
    trusted_core: TrustedCoreRequestV1,
    target: TargetSelectionRequestV1,
    out_slib: HostPathCarrier,
    diagnostics: DiagnosticOutputPolicyV1,
    emit: StageDumpPolicyV1,
    optimization: OptimizationMode,
}

impl ScoopcBuildRequestV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        current: CurrentConeRequestV1,
        direct_slibs: Vec<HostPathCarrier>,
        support_slibs: Vec<HostPathCarrier>,
        trusted_core: TrustedCoreRequestV1,
        target: TargetSelectionRequestV1,
        out_slib: HostPathCarrier,
        diagnostics: DiagnosticOutputPolicyV1,
        emit: StageDumpPolicyV1,
    ) -> Result<Self, ProtocolValidationError> {
        validate_build_shape(&current, &direct_slibs, &support_slibs, &trusted_core)?;
        Ok(Self {
            current,
            direct_slibs,
            support_slibs,
            trusted_core,
            target,
            out_slib,
            diagnostics,
            emit,
            optimization: OptimizationMode::Debug,
        })
    }

    pub fn with_optimization(mut self, optimization: OptimizationMode) -> Self {
        self.optimization = optimization;
        self
    }

    pub const fn optimization(&self) -> OptimizationMode {
        self.optimization
    }

    pub fn current(&self) -> &CurrentConeRequestV1 {
        &self.current
    }

    pub fn direct_slibs(&self) -> &[HostPathCarrier] {
        &self.direct_slibs
    }

    pub fn support_slibs(&self) -> &[HostPathCarrier] {
        &self.support_slibs
    }

    pub fn trusted_core(&self) -> &TrustedCoreRequestV1 {
        &self.trusted_core
    }

    pub fn target(&self) -> &TargetSelectionRequestV1 {
        &self.target
    }

    pub fn out_slib(&self) -> &HostPathCarrier {
        &self.out_slib
    }

    pub const fn diagnostics(&self) -> DiagnosticOutputPolicyV1 {
        self.diagnostics
    }

    pub const fn emit(&self) -> &StageDumpPolicyV1 {
        &self.emit
    }
}

impl WireEncode for ScoopcBuildRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.current.encode(encoder)?;
        encoder.field(2)?;
        encode_paths(encoder, &self.direct_slibs)?;
        encoder.field(3)?;
        encode_paths(encoder, &self.support_slibs)?;
        encoder.field(4)?;
        self.trusted_core.encode(encoder)?;
        encoder.field(5)?;
        self.target.encode(encoder)?;
        encoder.field(6)?;
        self.out_slib.encode(encoder)?;
        encoder.field(7)?;
        self.diagnostics.encode(encoder)?;
        encoder.field(8)?;
        self.emit.encode(encoder)?;
        encoder.field(9)?;
        self.optimization.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopcRequestEnvelopeV1 {
    request_id: RequestCorrelationId,
    build: ScoopcBuildRequestV1,
}

impl ScoopcRequestEnvelopeV1 {
    pub const fn new(request_id: RequestCorrelationId, build: ScoopcBuildRequestV1) -> Self {
        Self { request_id, build }
    }

    pub const fn request_id(&self) -> RequestCorrelationId {
        self.request_id
    }

    pub const fn build(&self) -> &ScoopcBuildRequestV1 {
        &self.build
    }
}

impl WireEncode for ScoopcRequestEnvelopeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        encoder.bytes(REQUEST_MAGIC)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(PROTOCOL_VERSION))?;
        encoder.field(3)?;
        self.request_id.encode(encoder)?;
        encoder.field(4)?;
        self.build.encode(encoder)
    }
}

mod decode;
#[cfg(test)]
use decode::DecodedCurrentConeRequestV1;
pub(crate) use decode::DecodedScoopcRequestEnvelopeV1;

fn validate_build_shape(
    current: &CurrentConeRequestV1,
    direct_slibs: &[HostPathCarrier],
    support_slibs: &[HostPathCarrier],
    trusted_core: &TrustedCoreRequestV1,
) -> Result<(), ProtocolValidationError> {
    match (current, trusted_core) {
        (CurrentConeRequestV1::ManifestRoot { .. }, _) => Ok(()),
        (CurrentConeRequestV1::SingleFile { .. }, TrustedCoreRequestV1::ArtifactSlot { .. }) => {
            if direct_slibs.is_empty() && support_slibs.is_empty() {
                Ok(())
            } else {
                Err(ProtocolValidationError::SingleFileHasDependencies)
            }
        }
        _ => Err(ProtocolValidationError::InvalidCurrentCoreCombination),
    }
}

fn validate_paths(
    paths: Vec<DecodedHostPathCarrier>,
) -> Result<Vec<HostPathCarrier>, ProtocolValidationError> {
    paths
        .into_iter()
        .map(|path| path.validate().map_err(ProtocolValidationError::HostPath))
        .collect()
}

fn encode_paths(
    encoder: &mut Encoder,
    paths: &[HostPathCarrier],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(paths.len() as u64)?;
    for path in paths {
        path.encode(encoder)?;
    }
    Ok(())
}

fn encode_decoded_paths(
    encoder: &mut Encoder,
    paths: &[DecodedHostPathCarrier],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(paths.len() as u64)?;
    for path in paths {
        path.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
