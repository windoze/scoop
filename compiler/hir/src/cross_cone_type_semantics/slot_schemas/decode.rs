use std::fmt;

use scoop_identity::{
    DecodedPersistentId, PersistentDispatchSlotId, PersistentExactTypeId, PersistentIdResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath};

use super::{
    CanonicalInheritanceSlotSchemasV1, InheritanceSlotSchemaBuildError,
    InheritanceSlotSchemaRoleV1, InheritanceSlotSchemaV1, wire,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedInheritanceSlotSchemaRoleV1 {
    ClassVtable,
    Interface {
        interface_exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}
impl DecodedInheritanceSlotSchemaRoleV1 {
    fn resolve<R: PersistentIdResolver<PersistentExactTypeId>>(
        self,
        resolver: &mut R,
    ) -> Result<InheritanceSlotSchemaRoleV1, R::Error> {
        match self {
            Self::ClassVtable => Ok(InheritanceSlotSchemaRoleV1::ClassVtable),
            Self::Interface { interface_exact } => resolver
                .resolve(interface_exact)
                .map(|interface_exact| InheritanceSlotSchemaRoleV1::Interface { interface_exact }),
        }
    }
}
impl WireEncode for DecodedInheritanceSlotSchemaRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ClassVtable => wire::tag(encoder, 1, 1),
            Self::Interface { interface_exact } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                interface_exact.encode(encoder)
            }
        }
    }
}
impl WireDecode for DecodedInheritanceSlotSchemaRoleV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::ClassVtable)
            }
            2 => {
                wire::expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(|interface_exact| Self::Interface { interface_exact })
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInheritanceSlotSchemaV1 {
    role: DecodedInheritanceSlotSchemaRoleV1,
    slots: Vec<DecodedPersistentId<PersistentDispatchSlotId>>,
}
impl DecodedInheritanceSlotSchemaV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<InheritanceSlotSchemaV1, InheritanceSlotSchemaResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
    {
        let root = WirePath::root();

        let role = self
            .role
            .resolve(resolver)
            .map_err(InheritanceSlotSchemaResolutionError::Identity)?;
        let mut slots = Vec::new();
        scoop_wire::allocation::try_reserve(&mut slots, self.slots.len(), &root)
            .map_err(InheritanceSlotSchemaResolutionError::Resource)?;

        let mut seen = std::collections::BTreeSet::new();
        for (position, slot) in self.slots.into_iter().enumerate() {
            let slot = resolver
                .resolve(slot)
                .map_err(InheritanceSlotSchemaResolutionError::Identity)?;
            if !seen.insert(slot) {
                return Err(InheritanceSlotSchemaResolutionError::Schema(
                    InheritanceSlotSchemaBuildError::DuplicateSlot { position, slot },
                ));
            }
            slots.push(slot);
        }
        Ok(InheritanceSlotSchemaV1 { role, slots })
    }
}
impl WireEncode for DecodedInheritanceSlotSchemaV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.role.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.slots)
    }
}
impl WireDecode for DecodedInheritanceSlotSchemaV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            role: decoder.field(1, DecodedInheritanceSlotSchemaRoleV1::decode)?,
            slots: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInheritanceSlotSchemasV1 {
    records: Vec<DecodedInheritanceSlotSchemaV1>,
}
impl DecodedCanonicalInheritanceSlotSchemasV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalInheritanceSlotSchemasV1, InheritanceSlotSchemaResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
    {
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), &WirePath::root())
            .map_err(InheritanceSlotSchemaResolutionError::Resource)?;
        for decoded in self.records {
            let record = decoded.resolve(resolver)?;
            if records
                .last()
                .is_some_and(|previous: &InheritanceSlotSchemaV1| previous.role() >= record.role())
            {
                return Err(InheritanceSlotSchemaResolutionError::Schema(
                    InheritanceSlotSchemaBuildError::RoleOrder {
                        index: records.len(),
                    },
                ));
            }
            records.push(record);
        }
        Ok(CanonicalInheritanceSlotSchemasV1 { records })
    }
}
impl WireEncode for DecodedCanonicalInheritanceSlotSchemasV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalInheritanceSlotSchemasV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedInheritanceSlotSchemaV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Debug)]
pub enum InheritanceSlotSchemaResolutionError<E> {
    Resource(WireError),
    Identity(E),
    Schema(InheritanceSlotSchemaBuildError),
}
impl<E: fmt::Display> fmt::Display for InheritanceSlotSchemaResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Schema(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceSlotSchemaResolutionError<E> {}
