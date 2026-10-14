use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::path::DecodedHostPathCarrier;
use crate::{HostPathCarrier, ProtocolValidationError};

/// A request either selects tools or carries the parent's completed discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetSelectionRequestV1 {
    canonical_triple: String,
    toolchain: ToolchainSelection<HostPathCarrier>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ToolchainSelection<P> {
    Discover {
        compiler: Option<P>,
        native_sysroot: Option<P>,
    },
    Resolved(Vec<u8>),
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
            toolchain: ToolchainSelection::Discover {
                compiler: None,
                native_sysroot: None,
            },
        })
    }

    pub fn with_c_toolchain(
        mut self,
        compiler: Option<HostPathCarrier>,
        native_sysroot: Option<HostPathCarrier>,
    ) -> Self {
        self.toolchain = ToolchainSelection::Discover {
            compiler,
            native_sysroot,
        };
        self
    }

    pub fn with_resolved_toolchain(mut self, record: Vec<u8>) -> Self {
        self.toolchain = ToolchainSelection::Resolved(record);
        self
    }

    pub fn canonical_triple(&self) -> &str {
        &self.canonical_triple
    }

    pub fn compiler(&self) -> Option<&HostPathCarrier> {
        match &self.toolchain {
            ToolchainSelection::Discover { compiler, .. } => compiler.as_ref(),
            ToolchainSelection::Resolved(_) => None,
        }
    }

    pub fn native_sysroot(&self) -> Option<&HostPathCarrier> {
        match &self.toolchain {
            ToolchainSelection::Discover { native_sysroot, .. } => native_sysroot.as_ref(),
            ToolchainSelection::Resolved(_) => None,
        }
    }

    pub fn resolved_toolchain(&self) -> Option<&[u8]> {
        match &self.toolchain {
            ToolchainSelection::Resolved(record) => Some(record),
            ToolchainSelection::Discover { .. } => None,
        }
    }
}

impl WireEncode for TargetSelectionRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, &self.canonical_triple, &self.toolchain)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedTargetSelectionRequestV1 {
    triple: String,
    toolchain: ToolchainSelection<DecodedHostPathCarrier>,
}

impl DecodedTargetSelectionRequestV1 {
    pub(super) fn validate(self) -> Result<TargetSelectionRequestV1, ProtocolValidationError> {
        let request = TargetSelectionRequestV1::new(self.triple)?;
        Ok(match self.toolchain {
            ToolchainSelection::Resolved(record) => request.with_resolved_toolchain(record),
            ToolchainSelection::Discover {
                compiler,
                native_sysroot,
            } => request.with_c_toolchain(
                compiler
                    .map(DecodedHostPathCarrier::validate)
                    .transpose()
                    .map_err(ProtocolValidationError::HostPath)?,
                native_sysroot
                    .map(DecodedHostPathCarrier::validate)
                    .transpose()
                    .map_err(ProtocolValidationError::HostPath)?,
            ),
        })
    }
}

impl WireEncode for DecodedTargetSelectionRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, &self.triple, &self.toolchain)
    }
}

fn encode(
    encoder: &mut Encoder,
    triple: &str,
    toolchain: &ToolchainSelection<impl WireEncode>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.text(triple)?;
    encoder.field(2)?;
    match toolchain {
        ToolchainSelection::Discover {
            compiler,
            native_sysroot,
        } => {
            encoder.array(3)?;
            encoder.unsigned(0)?;
            encode_path(encoder, compiler)?;
            encode_path(encoder, native_sysroot)
        }
        ToolchainSelection::Resolved(record) => {
            encoder.array(2)?;
            encoder.unsigned(1)?;
            encoder.bytes(record)
        }
    }
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
        decoder.expect_map(2)?;
        Ok(Self {
            triple: decoder.field(1, Decoder::owned_text)?,
            toolchain: decoder.field(2, |d| {
                let count = d.array()?;
                match d.index(0, Decoder::unsigned)? {
                    0 => {
                        crate::framing::expect_sum_length(d, count, 3)?;
                        Ok(ToolchainSelection::Discover {
                            compiler: d.index(1, decode_path)?,
                            native_sysroot: d.index(2, decode_path)?,
                        })
                    }
                    1 => {
                        crate::framing::expect_sum_length(d, count, 2)?;
                        Ok(ToolchainSelection::Resolved(
                            d.index(1, Decoder::owned_bytes)?,
                        ))
                    }
                    tag => Err(crate::framing::unknown_tag(d, tag)),
                }
            })?,
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
