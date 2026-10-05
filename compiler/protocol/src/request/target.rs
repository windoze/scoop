use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::path::DecodedHostPathCarrier;
use crate::{HostPathCarrier, ProtocolValidationError};

/// Same-host tool locators are transported separately from artifact identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetSelectionRequestV1 {
    canonical_triple: String,
    compiler: Option<HostPathCarrier>,
    native_sysroot: Option<HostPathCarrier>,
}

impl TargetSelectionRequestV1 {
    pub fn new(canonical_triple: String) -> Result<Self, ProtocolValidationError> {
        if canonical_triple.is_empty()
            || canonical_triple.len() > 255
            || !canonical_triple
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && byte != b'/' && byte != b'\\')
        {
            return Err(ProtocolValidationError::InvalidTargetTriple);
        }
        Ok(Self {
            canonical_triple,
            compiler: None,
            native_sysroot: None,
        })
    }

    pub fn with_c_toolchain(
        mut self,
        compiler: Option<HostPathCarrier>,
        native_sysroot: Option<HostPathCarrier>,
    ) -> Self {
        self.compiler = compiler;
        self.native_sysroot = native_sysroot;
        self
    }

    pub fn canonical_triple(&self) -> &str {
        &self.canonical_triple
    }

    pub const fn compiler(&self) -> Option<&HostPathCarrier> {
        self.compiler.as_ref()
    }

    pub const fn native_sysroot(&self) -> Option<&HostPathCarrier> {
        self.native_sysroot.as_ref()
    }
}

impl WireEncode for TargetSelectionRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(
            encoder,
            &self.canonical_triple,
            &self.compiler,
            &self.native_sysroot,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedTargetSelectionRequestV1 {
    triple: String,
    compiler: Option<DecodedHostPathCarrier>,
    native_sysroot: Option<DecodedHostPathCarrier>,
}

impl DecodedTargetSelectionRequestV1 {
    pub(super) fn validate(self) -> Result<TargetSelectionRequestV1, ProtocolValidationError> {
        Ok(
            TargetSelectionRequestV1::new(self.triple)?.with_c_toolchain(
                self.compiler
                    .map(DecodedHostPathCarrier::validate)
                    .transpose()
                    .map_err(ProtocolValidationError::HostPath)?,
                self.native_sysroot
                    .map(DecodedHostPathCarrier::validate)
                    .transpose()
                    .map_err(ProtocolValidationError::HostPath)?,
            ),
        )
    }
}

impl WireEncode for DecodedTargetSelectionRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, &self.triple, &self.compiler, &self.native_sysroot)
    }
}

fn encode(
    encoder: &mut Encoder,
    triple: &str,
    compiler: &Option<impl WireEncode>,
    native_sysroot: &Option<impl WireEncode>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    encoder.text(triple)?;
    encoder.field(2)?;
    encode_path(encoder, compiler)?;
    encoder.field(3)?;
    encode_path(encoder, native_sysroot)
}

fn encode_path(
    encoder: &mut Encoder,
    path: &Option<impl WireEncode>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(u64::from(path.is_some()))?;
    if let Some(path) = path {
        path.encode(encoder)?;
    }
    Ok(())
}

impl WireDecode for DecodedTargetSelectionRequestV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            triple: decoder.field(1, Decoder::owned_text)?,
            compiler: decoder.field(2, decode_path)?,
            native_sysroot: decoder.field(3, decode_path)?,
        })
    }
}

fn decode_path(decoder: &mut Decoder<'_>) -> Result<Option<DecodedHostPathCarrier>, WireError> {
    let count = decoder.array()?;
    if count == 0 {
        return Ok(None);
    }
    crate::framing::expect_sum_length(decoder, count, 1)?;
    decoder.index(0, DecodedHostPathCarrier::decode).map(Some)
}
