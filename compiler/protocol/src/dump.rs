use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::path::DecodedHostPathCarrier;
use crate::{HostPathCarrier, ProtocolValidationError};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StageDumpKindV1 {
    Ast,
    Hir,
    Mir,
    Lir,
}

impl StageDumpKindV1 {
    pub const ALL: [Self; 4] = [Self::Ast, Self::Hir, Self::Mir, Self::Lir];

    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Ast => "ast.txt",
            Self::Hir => "hir.txt",
            Self::Mir => "mir.txt",
            Self::Lir => "lir.txt",
        }
    }

    fn tag(self) -> u64 {
        self as u64 + 1
    }

    pub(crate) fn from_tag(tag: u64) -> Result<Self, ProtocolValidationError> {
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

/// A nonempty set of the four actual compiler observation stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageDumpSet(u8);

impl StageDumpSet {
    pub fn new(stages: &[StageDumpKindV1]) -> Result<Self, ProtocolValidationError> {
        if stages.is_empty() || !stages.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(ProtocolValidationError::InvalidDumpStages);
        }
        Ok(Self(
            stages
                .iter()
                .fold(0, |bits, stage| bits | (1 << *stage as u8)),
        ))
    }

    pub const fn one(stage: StageDumpKindV1) -> Self {
        Self(1 << stage as u8)
    }

    pub const fn all() -> Self {
        Self(0b1111)
    }

    pub const fn contains(self, stage: StageDumpKindV1) -> bool {
        self.0 & (1 << stage as u8) != 0
    }

    pub fn iter(self) -> impl Iterator<Item = StageDumpKindV1> {
        StageDumpKindV1::ALL
            .into_iter()
            .filter(move |stage| self.contains(*stage))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StageDumpPolicyV1 {
    None,
    Files {
        stages: StageDumpSet,
        directory: HostPathCarrier,
    },
}

impl WireEncode for StageDumpPolicyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Files { stages, directory } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                encoder.array(stages.iter().count() as u64)?;
                for stage in stages.iter() {
                    stage.encode(encoder)?;
                }
                encoder.field(2)?;
                directory.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DecodedStageDumpPolicyV1 {
    None,
    Files {
        stages: Vec<u64>,
        directory: DecodedHostPathCarrier,
    },
}

impl DecodedStageDumpPolicyV1 {
    pub(crate) fn validate(self) -> Result<StageDumpPolicyV1, ProtocolValidationError> {
        match self {
            Self::None => Ok(StageDumpPolicyV1::None),
            Self::Files { stages, directory } => {
                let stages = stages
                    .into_iter()
                    .map(StageDumpKindV1::from_tag)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(StageDumpPolicyV1::Files {
                    stages: StageDumpSet::new(&stages)?,
                    directory: directory
                        .validate()
                        .map_err(ProtocolValidationError::HostPath)?,
                })
            }
        }
    }
}

impl WireEncode for DecodedStageDumpPolicyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => StageDumpPolicyV1::None.encode(encoder),
            Self::Files { stages, directory } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                encoder.array(stages.len() as u64)?;
                for stage in stages {
                    encoder.unsigned(*stage)?;
                }
                encoder.field(2)?;
                directory.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedStageDumpPolicyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::None)
            }
            3 => {
                crate::framing::expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Files {
                    stages: decoder.field(1, |d| d.decode_array(|d, _| d.unsigned()))?,
                    directory: decoder.field(2, DecodedHostPathCarrier::decode)?,
                })
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

#[cfg(test)]
mod tests;
