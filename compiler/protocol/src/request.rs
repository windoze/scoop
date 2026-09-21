use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::path::DecodedHostPathCarrier;
use crate::{
    HostPathCarrier, MAX_INPUT_ARTIFACTS, PROTOCOL_VERSION, ProtocolValidationError,
    RequestCorrelationId,
};

const REQUEST_MAGIC: &[u8; 8] = b"SCOOPREQ";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurrentConeRequestV1 {
    ManifestRoot { root: HostPathCarrier },
    SingleFile { source: HostPathCarrier },
    TrustedCoreBootstrap,
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
            Self::TrustedCoreBootstrap => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(3)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetSelectionRequestV1 {
    canonical_triple: String,
}

impl TargetSelectionRequestV1 {
    pub fn new(canonical_triple: String) -> Result<Self, ProtocolValidationError> {
        if !valid_target_triple(&canonical_triple) {
            return Err(ProtocolValidationError::InvalidTargetTriple);
        }
        Ok(Self { canonical_triple })
    }

    pub fn canonical_triple(&self) -> &str {
        &self.canonical_triple
    }
}

impl WireEncode for TargetSelectionRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        encoder.text(&self.canonical_triple)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageDumpKindV1 {
    Ast,
    Hir,
    Mir,
    Lir,
}

impl StageDumpKindV1 {
    fn tag(self) -> u64 {
        match self {
            Self::Ast => 1,
            Self::Hir => 2,
            Self::Mir => 3,
            Self::Lir => 4,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, ProtocolValidationError> {
        match tag {
            1 => Ok(Self::Ast),
            2 => Ok(Self::Hir),
            3 => Ok(Self::Mir),
            4 => Ok(Self::Lir),
            _ => Err(ProtocolValidationError::UnknownEnumTag {
                kind: "stage dump",
                tag,
            }),
        }
    }
}

impl WireEncode for StageDumpKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.tag())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageDumpPolicyV1 {
    None,
    Stage(StageDumpKindV1),
}

impl WireEncode for StageDumpPolicyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Stage(stage) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                stage.encode(encoder)
            }
        }
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
        })
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

    pub const fn emit(&self) -> StageDumpPolicyV1 {
        self.emit
    }
}

impl WireEncode for ScoopcBuildRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
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
        self.emit.encode(encoder)
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedCurrentConeRequestV1 {
    ManifestRoot(DecodedHostPathCarrier),
    SingleFile(DecodedHostPathCarrier),
    TrustedCoreBootstrap,
}

impl DecodedCurrentConeRequestV1 {
    fn validate(self) -> Result<CurrentConeRequestV1, ProtocolValidationError> {
        match self {
            Self::ManifestRoot(root) => Ok(CurrentConeRequestV1::ManifestRoot {
                root: root.validate().map_err(ProtocolValidationError::HostPath)?,
            }),
            Self::SingleFile(source) => Ok(CurrentConeRequestV1::SingleFile {
                source: source
                    .validate()
                    .map_err(ProtocolValidationError::HostPath)?,
            }),
            Self::TrustedCoreBootstrap => Ok(CurrentConeRequestV1::TrustedCoreBootstrap),
        }
    }
}

impl WireEncode for DecodedCurrentConeRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ManifestRoot(root) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                root.encode(encoder)
            }
            Self::SingleFile(source) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                source.encode(encoder)
            }
            Self::TrustedCoreBootstrap => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(3)
            }
        }
    }
}

impl WireDecode for DecodedCurrentConeRequestV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedHostPathCarrier::decode)
                    .map(Self::ManifestRoot)
            }
            2 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedHostPathCarrier::decode)
                    .map(Self::SingleFile)
            }
            3 => {
                crate::framing::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::TrustedCoreBootstrap)
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedTrustedCoreRequestV1 {
    ArtifactSlot(DecodedHostPathCarrier),
    Bootstrap,
}

impl DecodedTrustedCoreRequestV1 {
    fn validate(self) -> Result<TrustedCoreRequestV1, ProtocolValidationError> {
        match self {
            Self::ArtifactSlot(artifact) => Ok(TrustedCoreRequestV1::ArtifactSlot {
                artifact: artifact
                    .validate()
                    .map_err(ProtocolValidationError::HostPath)?,
            }),
            Self::Bootstrap => Ok(TrustedCoreRequestV1::Bootstrap),
        }
    }
}

impl WireEncode for DecodedTrustedCoreRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ArtifactSlot(artifact) => {
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

impl WireDecode for DecodedTrustedCoreRequestV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedHostPathCarrier::decode)
                    .map(Self::ArtifactSlot)
            }
            2 => {
                crate::framing::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Bootstrap)
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedTargetSelectionRequestV1(String);

impl WireEncode for DecodedTargetSelectionRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        encoder.text(&self.0)
    }
}

impl WireDecode for DecodedTargetSelectionRequestV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        decoder.field(1, Decoder::owned_text).map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedScoopcBuildRequestV1 {
    current: DecodedCurrentConeRequestV1,
    direct_slibs: Vec<DecodedHostPathCarrier>,
    support_slibs: Vec<DecodedHostPathCarrier>,
    trusted_core: DecodedTrustedCoreRequestV1,
    target: DecodedTargetSelectionRequestV1,
    out_slib: DecodedHostPathCarrier,
    diagnostics: u64,
    emit: DecodedStageDumpPolicyV1,
}

impl DecodedScoopcBuildRequestV1 {
    fn validate(self) -> Result<ScoopcBuildRequestV1, ProtocolValidationError> {
        let current = self.current.validate()?;
        let direct_slibs = validate_paths(self.direct_slibs)?;
        let support_slibs = validate_paths(self.support_slibs)?;
        let trusted_core = self.trusted_core.validate()?;
        let target = TargetSelectionRequestV1::new(self.target.0)?;
        let out_slib = self
            .out_slib
            .validate()
            .map_err(ProtocolValidationError::HostPath)?;
        let diagnostics = DiagnosticOutputPolicyV1::from_tag(self.diagnostics)?;
        let emit = self.emit.validate()?;
        ScoopcBuildRequestV1::new(
            current,
            direct_slibs,
            support_slibs,
            trusted_core,
            target,
            out_slib,
            diagnostics,
            emit,
        )
    }
}

impl WireEncode for DecodedScoopcBuildRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.current.encode(encoder)?;
        encoder.field(2)?;
        encode_decoded_paths(encoder, &self.direct_slibs)?;
        encoder.field(3)?;
        encode_decoded_paths(encoder, &self.support_slibs)?;
        encoder.field(4)?;
        self.trusted_core.encode(encoder)?;
        encoder.field(5)?;
        self.target.encode(encoder)?;
        encoder.field(6)?;
        self.out_slib.encode(encoder)?;
        encoder.field(7)?;
        encoder.unsigned(self.diagnostics)?;
        encoder.field(8)?;
        self.emit.encode(encoder)
    }
}

impl WireDecode for DecodedScoopcBuildRequestV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        let current = decoder.field(1, DecodedCurrentConeRequestV1::decode)?;
        let direct_slibs = decoder.field(2, |decoder| {
            decoder.decode_array(|decoder, _| DecodedHostPathCarrier::decode(decoder))
        })?;
        let support_slibs = decoder.field(3, |decoder| {
            decoder.decode_array(|decoder, _| DecodedHostPathCarrier::decode(decoder))
        })?;
        let trusted_core = decoder.field(4, DecodedTrustedCoreRequestV1::decode)?;
        let target = decoder.field(5, DecodedTargetSelectionRequestV1::decode)?;
        let out_slib = decoder.field(6, DecodedHostPathCarrier::decode)?;
        let diagnostics = decoder.field(7, Decoder::unsigned)?;
        let emit = decoder.field(8, DecodedStageDumpPolicyV1::decode)?;
        Ok(Self {
            current,
            direct_slibs,
            support_slibs,
            trusted_core,
            target,
            out_slib,
            diagnostics,
            emit,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedStageDumpPolicyV1 {
    None,
    Stage(u64),
}

impl DecodedStageDumpPolicyV1 {
    fn validate(self) -> Result<StageDumpPolicyV1, ProtocolValidationError> {
        match self {
            Self::None => Ok(StageDumpPolicyV1::None),
            Self::Stage(tag) => Ok(StageDumpPolicyV1::Stage(StageDumpKindV1::from_tag(tag)?)),
        }
    }
}

impl WireEncode for DecodedStageDumpPolicyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Stage(tag) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                encoder.unsigned(*tag)
            }
        }
    }
}

impl WireDecode for DecodedStageDumpPolicyV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::None)
            }
            2 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                decoder.field(1, Decoder::unsigned).map(Self::Stage)
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedScoopcRequestEnvelopeV1 {
    magic: Vec<u8>,
    version: u32,
    request_id: Vec<u8>,
    build: DecodedScoopcBuildRequestV1,
}

impl DecodedScoopcRequestEnvelopeV1 {
    pub(crate) fn validate(self) -> Result<ScoopcRequestEnvelopeV1, ProtocolValidationError> {
        if self.magic.as_slice() != REQUEST_MAGIC {
            return Err(ProtocolValidationError::InvalidRequestMagic);
        }
        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolValidationError::UnsupportedVersion(self.version));
        }
        Ok(ScoopcRequestEnvelopeV1::new(
            RequestCorrelationId::from_vec(self.request_id)?,
            self.build.validate()?,
        ))
    }
}

impl WireEncode for DecodedScoopcRequestEnvelopeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        encoder.bytes(&self.magic)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.version))?;
        encoder.field(3)?;
        encoder.bytes(&self.request_id)?;
        encoder.field(4)?;
        self.build.encode(encoder)
    }
}

impl WireDecode for DecodedScoopcRequestEnvelopeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        let magic = decoder.field(1, Decoder::owned_bytes)?;
        let version = decoder.field(2, Decoder::u32)?;
        let request_id = decoder.field(3, Decoder::owned_bytes)?;
        let build = decoder.field(4, DecodedScoopcBuildRequestV1::decode)?;
        Ok(Self {
            magic,
            version,
            request_id,
            build,
        })
    }
}

fn validate_build_shape(
    current: &CurrentConeRequestV1,
    direct_slibs: &[HostPathCarrier],
    support_slibs: &[HostPathCarrier],
    trusted_core: &TrustedCoreRequestV1,
) -> Result<(), ProtocolValidationError> {
    if direct_slibs.len() > MAX_INPUT_ARTIFACTS {
        return Err(ProtocolValidationError::TooManyInputs {
            role: "direct",
            actual: direct_slibs.len(),
        });
    }
    if support_slibs.len() > MAX_INPUT_ARTIFACTS {
        return Err(ProtocolValidationError::TooManyInputs {
            role: "support",
            actual: support_slibs.len(),
        });
    }
    match (current, trusted_core) {
        (CurrentConeRequestV1::ManifestRoot { .. }, TrustedCoreRequestV1::ArtifactSlot { .. }) => {
            Ok(())
        }
        (CurrentConeRequestV1::SingleFile { .. }, TrustedCoreRequestV1::ArtifactSlot { .. }) => {
            if direct_slibs.is_empty() && support_slibs.is_empty() {
                Ok(())
            } else {
                Err(ProtocolValidationError::SingleFileHasDependencies)
            }
        }
        (
            CurrentConeRequestV1::TrustedCoreBootstrap | CurrentConeRequestV1::ManifestRoot { .. },
            TrustedCoreRequestV1::Bootstrap,
        ) => {
            if direct_slibs.is_empty() && support_slibs.is_empty() {
                Ok(())
            } else {
                Err(ProtocolValidationError::BootstrapHasDependencies)
            }
        }
        _ => Err(ProtocolValidationError::InvalidCurrentCoreCombination),
    }
}

fn valid_target_triple(triple: &str) -> bool {
    !triple.is_empty()
        && triple.len() <= 255
        && triple
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'/' && byte != b'\\')
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
mod tests {
    use std::path::Path;

    use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

    use super::*;

    fn path(value: &str) -> HostPathCarrier {
        HostPathCarrier::from_path(Path::new(value)).unwrap()
    }

    fn target() -> TargetSelectionRequestV1 {
        TargetSelectionRequestV1::new("aarch64-apple-darwin".to_owned()).unwrap()
    }

    #[test]
    fn core_manifest_request_round_trips_without_a_default_source_slot() {
        let request = ScoopcRequestEnvelopeV1::new(
            RequestCorrelationId::from_array([72; 16]),
            ScoopcBuildRequestV1::new(
                CurrentConeRequestV1::ManifestRoot {
                    root: path("edited-library"),
                },
                Vec::new(),
                Vec::new(),
                TrustedCoreRequestV1::Bootstrap,
                target(),
                path("custom-output/core.slib"),
                DiagnosticOutputPolicyV1::Structured,
                StageDumpPolicyV1::None,
            )
            .unwrap(),
        );
        let decoded = decode_canonical::<DecodedScoopcRequestEnvelopeV1>(
            &encode(&request).unwrap(),
            DecodeLimits::M23_DEFAULT,
        )
        .unwrap()
        .validate()
        .unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn request_constructor_closes_core_bootstrap_combinations() {
        let error = ScoopcBuildRequestV1::new(
            CurrentConeRequestV1::TrustedCoreBootstrap,
            vec![path("dependency.slib")],
            Vec::new(),
            TrustedCoreRequestV1::Bootstrap,
            target(),
            path("core.slib"),
            DiagnosticOutputPolicyV1::Structured,
            StageDumpPolicyV1::None,
        )
        .unwrap_err();
        assert_eq!(error, ProtocolValidationError::BootstrapHasDependencies);

        let error = ScoopcBuildRequestV1::new(
            CurrentConeRequestV1::SingleFile {
                source: path("main.scoop"),
            },
            Vec::new(),
            Vec::new(),
            TrustedCoreRequestV1::Bootstrap,
            target(),
            path("main.slib"),
            DiagnosticOutputPolicyV1::Human,
            StageDumpPolicyV1::Stage(StageDumpKindV1::Hir),
        )
        .unwrap_err();
        assert_eq!(
            error,
            ProtocolValidationError::InvalidCurrentCoreCombination
        );

        let error = ScoopcBuildRequestV1::new(
            CurrentConeRequestV1::SingleFile {
                source: path("main.scoop"),
            },
            vec![path("dependency.slib")],
            Vec::new(),
            TrustedCoreRequestV1::ArtifactSlot {
                artifact: path("core.slib"),
            },
            target(),
            path("main.slib"),
            DiagnosticOutputPolicyV1::Human,
            StageDumpPolicyV1::None,
        )
        .unwrap_err();
        assert_eq!(error, ProtocolValidationError::SingleFileHasDependencies);

        let input = path("dependency.slib");
        let error = ScoopcBuildRequestV1::new(
            CurrentConeRequestV1::SingleFile {
                source: path("main.scoop"),
            },
            vec![input; MAX_INPUT_ARTIFACTS + 1],
            Vec::new(),
            TrustedCoreRequestV1::ArtifactSlot {
                artifact: path("core.slib"),
            },
            target(),
            path("main.slib"),
            DiagnosticOutputPolicyV1::Human,
            StageDumpPolicyV1::None,
        )
        .unwrap_err();
        assert_eq!(
            error,
            ProtocolValidationError::TooManyInputs {
                role: "direct",
                actual: MAX_INPUT_ARTIFACTS + 1,
            }
        );
    }

    struct RawCurrentCone {
        fields: u64,
        tag: u64,
    }

    impl WireEncode for RawCurrentCone {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.map(self.fields)?;
            encoder.field(0)?;
            encoder.unsigned(self.tag)
        }
    }

    #[test]
    fn current_cone_reader_distinguishes_unknown_tag_and_wrong_sum_length() {
        let error = decode_canonical::<DecodedCurrentConeRequestV1>(
            &encode(&RawCurrentCone { fields: 1, tag: 9 }).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 9 });

        let error = decode_canonical::<DecodedCurrentConeRequestV1>(
            &encode(&RawCurrentCone { fields: 1, tag: 1 }).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 2,
                actual: 1,
            }
        );
    }
}
