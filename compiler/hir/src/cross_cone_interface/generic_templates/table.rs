use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedExportGenericCallableBodyV1, ExportGenericCallableBodyV1, GenericCallableBodyIndexError,
    GenericCallableBodyResolutionError, IndexedExportGenericCallableBodyV1,
};
use crate::{DefaultCallableDeclarationV1, DefaultStatementReferenceResolver};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalExportGenericCallableBodiesV1 {
    records: Vec<ExportGenericCallableBodyV1>,
}

impl CanonicalExportGenericCallableBodiesV1 {
    pub fn try_new(
        mut records: Vec<ExportGenericCallableBodyV1>,
    ) -> Result<Self, GenericCallableBodyTableError> {
        records.sort_unstable_by_key(ExportGenericCallableBodyV1::owner);
        require_order(&records)?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[ExportGenericCallableBodyV1] {
        &self.records
    }

    pub fn get(&self, owner: DefaultCallableDeclarationV1) -> Option<&ExportGenericCallableBodyV1> {
        self.records
            .binary_search_by_key(&owner, ExportGenericCallableBodyV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn index_locals(
        &self,
    ) -> Result<IndexedExportGenericCallableBodiesV1<'_>, GenericCallableBodyIndexError> {
        let records = self
            .records
            .iter()
            .map(ExportGenericCallableBodyV1::index_locals)
            .collect::<Result<_, _>>()?;
        Ok(IndexedExportGenericCallableBodiesV1 { records })
    }
}

pub struct IndexedExportGenericCallableBodiesV1<'a> {
    records: Vec<IndexedExportGenericCallableBodyV1<'a>>,
}

impl WireEncode for IndexedExportGenericCallableBodiesV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExportGenericCallableBodiesV1 {
    records: Vec<DecodedExportGenericCallableBodyV1>,
}

impl DecodedCanonicalExportGenericCallableBodiesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExportGenericCallableBodiesV1, GenericCallableBodiesResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        let records = self
            .records
            .into_iter()
            .enumerate()
            .map(|(index, record)| {
                record.resolve(resolver).map_err(|source| {
                    GenericCallableBodiesResolutionError::Body {
                        index,
                        source: Box::new(source),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        require_order(&records).map_err(GenericCallableBodiesResolutionError::Table)?;
        Ok(CanonicalExportGenericCallableBodiesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalExportGenericCallableBodiesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalExportGenericCallableBodiesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExportGenericCallableBodyV1::decode(decoder))
            .map(|records| Self { records })
    }
}

fn require_order(
    records: &[ExportGenericCallableBodyV1],
) -> Result<(), GenericCallableBodyTableError> {
    for (index, pair) in records.windows(2).enumerate() {
        if pair[0].owner() >= pair[1].owner() {
            return Err(GenericCallableBodyTableError {
                index: index + 1,
                owner: pair[1].owner(),
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericCallableBodyTableError {
    pub index: usize,
    pub owner: DefaultCallableDeclarationV1,
}

impl fmt::Display for GenericCallableBodyTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generic body {} for {:?} duplicates or precedes its previous owner",
            self.index, self.owner
        )
    }
}
impl std::error::Error for GenericCallableBodyTableError {}

#[derive(Debug)]
pub enum GenericCallableBodiesResolutionError<E> {
    Body {
        index: usize,
        source: Box<GenericCallableBodyResolutionError<E>>,
    },
    Table(GenericCallableBodyTableError),
}

impl<E: fmt::Display> fmt::Display for GenericCallableBodiesResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Body { index, source } => {
                write!(formatter, "invalid generic body {index}: {source}")
            }
            Self::Table(source) => source.fmt(formatter),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for GenericCallableBodiesResolutionError<E> {}
