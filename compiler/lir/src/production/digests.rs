use std::fmt;

use scoop_identity::{
    CborIdentityRecord, DecodedCborIdentityRecord, DecodedDigestNodeKey, DecodedPersistentId,
    DigestKind, DigestNodeId, DigestNodeKey,
};
use scoop_wire::{Decoder, Encoder, HashError, WireDecode, WireEncode, WireError};

mod validation;
pub use validation::DigestPlanError;
use validation::{first_duplicate_input, validate_node_order, validate_plan};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DigestInputRefV1 {
    SourceSignature(DigestNodeId),
    Layout(DigestNodeId),
    Scan(DigestNodeId),
    ObjectSupport(DigestNodeId),
    ObjectDefinition(DigestNodeId),
    StackmapRecord(DigestNodeId),
    RuntimeImage(DigestNodeId),
}

impl DigestInputRefV1 {
    pub fn from_node(node: &DigestNodeV1) -> Self {
        Self::from_parts(node.kind(), node.id())
    }

    const fn from_parts(kind: DigestKind, id: DigestNodeId) -> Self {
        match kind {
            DigestKind::SourceSignature => Self::SourceSignature(id),
            DigestKind::Layout => Self::Layout(id),
            DigestKind::Scan => Self::Scan(id),
            DigestKind::ObjectSupport => Self::ObjectSupport(id),
            DigestKind::ObjectDefinition => Self::ObjectDefinition(id),
            DigestKind::StackmapRecord => Self::StackmapRecord(id),
            DigestKind::RuntimeImage => Self::RuntimeImage(id),
        }
    }

    pub const fn kind(self) -> DigestKind {
        match self {
            Self::SourceSignature(_) => DigestKind::SourceSignature,
            Self::Layout(_) => DigestKind::Layout,
            Self::Scan(_) => DigestKind::Scan,
            Self::ObjectSupport(_) => DigestKind::ObjectSupport,
            Self::ObjectDefinition(_) => DigestKind::ObjectDefinition,
            Self::StackmapRecord(_) => DigestKind::StackmapRecord,
            Self::RuntimeImage(_) => DigestKind::RuntimeImage,
        }
    }

    pub const fn node(self) -> DigestNodeId {
        match self {
            Self::SourceSignature(id)
            | Self::Layout(id)
            | Self::Scan(id)
            | Self::ObjectSupport(id)
            | Self::ObjectDefinition(id)
            | Self::StackmapRecord(id)
            | Self::RuntimeImage(id) => id,
        }
    }

    const fn sort_key(self) -> (u32, DigestNodeId) {
        (self.kind().tag(), self.node())
    }
}

impl WireEncode for DigestInputRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_value_sum(encoder, u64::from(self.kind().tag()), &self.node())
    }
}

#[derive(Debug)]
enum DecodedDigestInputRefV1 {
    SourceSignature(DecodedPersistentId<DigestNodeId>),
    Layout(DecodedPersistentId<DigestNodeId>),
    Scan(DecodedPersistentId<DigestNodeId>),
    ObjectSupport(DecodedPersistentId<DigestNodeId>),
    ObjectDefinition(DecodedPersistentId<DigestNodeId>),
    StackmapRecord(DecodedPersistentId<DigestNodeId>),
    RuntimeImage(DecodedPersistentId<DigestNodeId>),
}

impl DecodedDigestInputRefV1 {
    const fn kind(&self) -> DigestKind {
        match self {
            Self::SourceSignature(_) => DigestKind::SourceSignature,
            Self::Layout(_) => DigestKind::Layout,
            Self::Scan(_) => DigestKind::Scan,
            Self::ObjectSupport(_) => DigestKind::ObjectSupport,
            Self::ObjectDefinition(_) => DigestKind::ObjectDefinition,
            Self::StackmapRecord(_) => DigestKind::StackmapRecord,
            Self::RuntimeImage(_) => DigestKind::RuntimeImage,
        }
    }

    const fn decoded_node(&self) -> DecodedPersistentId<DigestNodeId> {
        match self {
            Self::SourceSignature(id)
            | Self::Layout(id)
            | Self::Scan(id)
            | Self::ObjectSupport(id)
            | Self::ObjectDefinition(id)
            | Self::StackmapRecord(id)
            | Self::RuntimeImage(id) => *id,
        }
    }
}

impl WireEncode for DecodedDigestInputRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_value_sum(encoder, u64::from(self.kind().tag()), &self.decoded_node())
    }
}

impl WireDecode for DecodedDigestInputRefV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => decode_id_variant(decoder, fields, Self::SourceSignature),
            2 => decode_id_variant(decoder, fields, Self::Layout),
            3 => decode_id_variant(decoder, fields, Self::Scan),
            5 => decode_id_variant(decoder, fields, Self::ObjectSupport),
            6 => decode_id_variant(decoder, fields, Self::ObjectDefinition),
            7 => decode_id_variant(decoder, fields, Self::StackmapRecord),
            10 => decode_id_variant(decoder, fields, Self::RuntimeImage),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DigestNodeV1 {
    identity: CborIdentityRecord<DigestNodeId, DigestNodeKey>,
    direct_inputs: Vec<DigestInputRefV1>,
}

impl DigestNodeV1 {
    pub fn new(
        key: DigestNodeKey,
        mut direct_inputs: Vec<DigestInputRefV1>,
    ) -> Result<Self, DigestNodeBuildError> {
        let identity = CborIdentityRecord::from_key(key).map_err(DigestNodeBuildError::Identity)?;
        direct_inputs.sort_unstable_by_key(|input| input.sort_key());
        if let Some(input) = first_duplicate_input(&direct_inputs) {
            return Err(DigestNodeBuildError::DuplicateInput {
                node: identity.id(),
                input,
            });
        }

        Ok(Self {
            identity,
            direct_inputs,
        })
    }

    pub const fn id(&self) -> DigestNodeId {
        self.identity.id()
    }

    pub fn key(&self) -> &DigestNodeKey {
        self.identity.key()
    }

    pub fn kind(&self) -> DigestKind {
        self.identity.key().kind()
    }

    pub fn direct_inputs(&self) -> &[DigestInputRefV1] {
        &self.direct_inputs
    }

    fn sort_key(&self) -> (u32, DigestNodeId) {
        (self.kind().tag(), self.id())
    }
}

impl WireEncode for DigestNodeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.identity.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.direct_inputs)
    }
}

#[derive(Debug)]
struct DecodedDigestNodeV1 {
    identity: DecodedCborIdentityRecord<DigestNodeId, DecodedDigestNodeKey>,
    direct_inputs: Vec<DecodedDigestInputRefV1>,
}

impl WireEncode for DecodedDigestNodeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.identity.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.direct_inputs)
    }
}

impl WireDecode for DecodedDigestNodeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            identity: decoder.field(1, DecodedCborIdentityRecord::decode)?,
            direct_inputs: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDigestInputRefV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DigestFinalizationPlanV1 {
    nodes: Vec<DigestNodeV1>,
}

impl DigestFinalizationPlanV1 {
    pub(crate) fn remove_safepoint_records(
        &mut self,
        removed: &std::collections::BTreeSet<DigestNodeId>,
    ) {
        if removed.is_empty() {
            return;
        }
        self.nodes.retain(|node| !removed.contains(&node.id()));
        for node in &mut self.nodes {
            node.direct_inputs
                .retain(|input| !removed.contains(&input.node()));
        }
    }

    pub fn new(
        mut nodes: Vec<DigestNodeV1>,
        foundation: &crate::ConeLirFoundation,
    ) -> Result<Self, DigestPlanBuildError> {
        nodes.sort_unstable_by_key(DigestNodeV1::sort_key);
        validate_node_order(&nodes).map_err(DigestPlanBuildError::Plan)?;
        validate_plan(&nodes, foundation).map_err(DigestPlanBuildError::Plan)?;
        Ok(Self { nodes })
    }

    pub fn nodes(&self) -> &[DigestNodeV1] {
        &self.nodes
    }

    pub fn validate_against(
        &self,
        foundation: &crate::ConeLirFoundation,
    ) -> Result<(), DigestPlanError> {
        validate_plan(&self.nodes, foundation)
    }
}

impl WireEncode for DigestFinalizationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.nodes)
    }
}

#[derive(Debug)]
pub struct DecodedDigestFinalizationPlanV1 {
    nodes: Vec<DecodedDigestNodeV1>,
}

impl WireEncode for DecodedDigestFinalizationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.nodes)
    }
}

impl WireDecode for DecodedDigestFinalizationPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedDigestNodeV1::decode(decoder))
            .map(|nodes| Self { nodes })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestNodeBuildError {
    Identity(HashError),
    DuplicateInput {
        node: DigestNodeId,
        input: DigestNodeId,
    },
}

impl fmt::Display for DigestNodeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid digest node: {self:?}")
    }
}

impl std::error::Error for DigestNodeBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestPlanBuildError {
    Plan(DigestPlanError),
}

impl fmt::Display for DigestPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong digest finalization plan: {self:?}"
        )
    }
}

impl std::error::Error for DigestPlanBuildError {}

fn encode_array<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn decode_sum_header(decoder: &mut Decoder<'_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_id_variant<T>(
    decoder: &mut Decoder<'_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<DigestNodeId>) -> T,
) -> Result<T, WireError> {
    if fields != 2 {
        return Err(WireError::new(
            scoop_wire::WireErrorKind::InvalidLength {
                expected: 2,
                actual: fields,
            },
            decoder.path().clone(),
            Some(decoder.position()),
        ));
    }
    decoder.field(1, DecodedPersistentId::decode).map(build)
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        scoop_wire::WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests;
