use std::fmt;

use scoop_identity::{DecodedPersistentId, PersistentExactTypeId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::cross_cone_type_semantics::wire;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalInheritanceModalityV1 {
    Final,
    Open,
    Abstract,
    Interface,
}
impl WireEncode for NominalInheritanceModalityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::tag(
            encoder,
            1,
            match self {
                Self::Final => 1,
                Self::Open => 2,
                Self::Abstract => 3,
                Self::Interface => 4,
            },
        )
    }
}
impl WireDecode for NominalInheritanceModalityV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::Final),
            2 => Ok(Self::Open),
            3 => Ok(Self::Abstract),
            4 => Ok(Self::Interface),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectClassBaseV1 {
    NoClassBase,
    ClassBase { exact: PersistentExactTypeId },
}
impl WireEncode for DirectClassBaseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoClassBase => wire::tag(encoder, 1, 1),
            Self::ClassBase { exact } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                exact.encode(encoder)
            }
        }
    }
}

/// The first four fields of an inheritance interface, grouped in memory.
/// The complete interface writes these fields directly, without a new wire
/// nesting layer. Construction does not prove the referenced graph complete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalInheritanceEdgesV1 {
    owner: PersistentExactTypeId,
    modality: NominalInheritanceModalityV1,
    direct_base: DirectClassBaseV1,
    direct_interfaces: Vec<PersistentExactTypeId>,
}
impl NominalInheritanceEdgesV1 {
    pub fn try_new(
        owner: PersistentExactTypeId,
        modality: NominalInheritanceModalityV1,
        direct_base: DirectClassBaseV1,
        mut direct_interfaces: Vec<PersistentExactTypeId>,
    ) -> Result<Self, InheritanceEdgeOrderError> {
        direct_interfaces.sort_unstable();
        Self::from_ordered(owner, modality, direct_base, direct_interfaces)
    }
    fn from_ordered(
        owner: PersistentExactTypeId,
        modality: NominalInheritanceModalityV1,
        direct_base: DirectClassBaseV1,
        direct_interfaces: Vec<PersistentExactTypeId>,
    ) -> Result<Self, InheritanceEdgeOrderError> {
        if let Some(index) = direct_interfaces
            .windows(2)
            .position(|pair| pair[0] >= pair[1])
        {
            return Err(InheritanceEdgeOrderError { index: index + 1 });
        }
        Ok(Self {
            owner,
            modality,
            direct_base,
            direct_interfaces,
        })
    }
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub const fn modality(&self) -> NominalInheritanceModalityV1 {
        self.modality
    }
    pub const fn direct_base(&self) -> DirectClassBaseV1 {
        self.direct_base
    }
    pub fn direct_interfaces(&self) -> &[PersistentExactTypeId] {
        &self.direct_interfaces
    }

    pub(super) fn encode_fields(
        &self,
        encoder: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.modality.encode(encoder)?;
        encoder.field(3)?;
        self.direct_base.encode(encoder)?;
        encoder.field(4)?;
        wire::sequence(encoder, &self.direct_interfaces)
    }
}
impl WireEncode for NominalInheritanceEdgesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        self.encode_fields(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDirectClassBaseV1 {
    NoClassBase,
    ClassBase {
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}
impl DecodedDirectClassBaseV1 {
    pub fn resolve<R: PersistentIdResolver<PersistentExactTypeId>>(
        self,
        resolver: &mut R,
    ) -> Result<DirectClassBaseV1, R::Error> {
        match self {
            Self::NoClassBase => Ok(DirectClassBaseV1::NoClassBase),
            Self::ClassBase { exact } => resolver
                .resolve(exact)
                .map(|exact| DirectClassBaseV1::ClassBase { exact }),
        }
    }
}
impl WireEncode for DecodedDirectClassBaseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoClassBase => wire::tag(encoder, 1, 1),
            Self::ClassBase { exact } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                exact.encode(encoder)
            }
        }
    }
}
impl WireDecode for DecodedDirectClassBaseV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::NoClassBase)
            }
            2 => {
                wire::expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(|exact| Self::ClassBase { exact })
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalInheritanceEdgesV1 {
    owner: DecodedPersistentId<PersistentExactTypeId>,
    modality: NominalInheritanceModalityV1,
    direct_base: DecodedDirectClassBaseV1,
    direct_interfaces: Vec<DecodedPersistentId<PersistentExactTypeId>>,
}
impl DecodedNominalInheritanceEdgesV1 {
    pub fn resolve_metered<R: PersistentIdResolver<PersistentExactTypeId>>(
        self,
        resolver: &mut R,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<NominalInheritanceEdgesV1, InheritanceEdgeResolutionError<R::Error>> {
        let path = scoop_wire::WirePath::root();
        let count = self.direct_interfaces.len() as u64;
        meter
            .check_semantic_depth(1, &path)
            .map_err(InheritanceEdgeResolutionError::Resource)?;
        meter
            .check_table_entries(count, &path)
            .map_err(InheritanceEdgeResolutionError::Resource)?;
        meter
            .charge_edges(
                count.saturating_add(u64::from(matches!(
                    self.direct_base,
                    DecodedDirectClassBaseV1::ClassBase { .. }
                ))),
                &path,
            )
            .map_err(InheritanceEdgeResolutionError::Resource)?;
        meter
            .charge_nodes(count.saturating_add(1), &path)
            .map_err(InheritanceEdgeResolutionError::Resource)?;
        meter
            .charge_collection_slots(count, &path)
            .map_err(InheritanceEdgeResolutionError::Resource)?;
        meter
            .charge_work(count.saturating_add(2), &path)
            .map_err(InheritanceEdgeResolutionError::Resource)?;
        self.resolve(resolver)
    }
    pub fn resolve<R: PersistentIdResolver<PersistentExactTypeId>>(
        self,
        resolver: &mut R,
    ) -> Result<NominalInheritanceEdgesV1, InheritanceEdgeResolutionError<R::Error>> {
        let owner = resolver
            .resolve(self.owner)
            .map_err(InheritanceEdgeResolutionError::Identity)?;
        let direct_base = self
            .direct_base
            .resolve(resolver)
            .map_err(InheritanceEdgeResolutionError::Identity)?;
        let interfaces = self
            .direct_interfaces
            .into_iter()
            .map(|id| resolver.resolve(id))
            .collect::<Result<Vec<_>, _>>()
            .map_err(InheritanceEdgeResolutionError::Identity)?;
        NominalInheritanceEdgesV1::from_ordered(owner, self.modality, direct_base, interfaces)
            .map_err(InheritanceEdgeResolutionError::Order)
    }
    pub(super) fn decode_fields(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            modality: decoder.field(2, NominalInheritanceModalityV1::decode)?,
            direct_base: decoder.field(3, DecodedDirectClassBaseV1::decode)?,
            direct_interfaces: decoder.field(4, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            })?,
        })
    }
}
impl WireDecode for DecodedNominalInheritanceEdgesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Self::decode_fields(decoder)
    }
}
impl DecodedNominalInheritanceEdgesV1 {
    pub(super) fn encode_fields(
        &self,
        encoder: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.modality.encode(encoder)?;
        encoder.field(3)?;
        self.direct_base.encode(encoder)?;
        encoder.field(4)?;
        wire::sequence(encoder, &self.direct_interfaces)
    }
}
impl WireEncode for DecodedNominalInheritanceEdgesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        self.encode_fields(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InheritanceEdgeOrderError {
    pub index: usize,
}
impl fmt::Display for InheritanceEdgeOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "duplicate or noncanonical direct interface at index {}",
            self.index
        )
    }
}
impl std::error::Error for InheritanceEdgeOrderError {}
#[derive(Debug)]
pub enum InheritanceEdgeResolutionError<E> {
    Resource(WireError),
    Identity(E),
    Order(InheritanceEdgeOrderError),
}
impl<E: fmt::Display> fmt::Display for InheritanceEdgeResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Order(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceEdgeResolutionError<E> {}
