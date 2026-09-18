use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedNominalInterfaceRecordV1, NominalInterfaceRecordResolutionError,
    NominalInterfaceRecordResolver, NominalInterfaceRecordV1, SourceNominalId,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalInterfacesV1 {
    records: Vec<NominalInterfaceRecordV1>,
}

impl CanonicalNominalInterfacesV1 {
    pub fn try_new(
        mut records: Vec<NominalInterfaceRecordV1>,
    ) -> Result<Self, NominalInterfaceSetBuildError> {
        records.sort_unstable_by_key(NominalInterfaceRecordV1::declaration);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].declaration() == pair[1].declaration())
        {
            return Err(NominalInterfaceSetBuildError::DuplicateDeclaration(
                pair[0].declaration(),
            ));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[NominalInterfaceRecordV1] {
        &self.records
    }

    pub fn get(&self, declaration: SourceNominalId) -> Option<&NominalInterfaceRecordV1> {
        self.records
            .binary_search_by_key(&declaration, NominalInterfaceRecordV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), NominalInterfaceSetSemanticValidationError<E>>
    where
        A: super::NominalInterfaceSemanticAuthority<E>,
    {
        for (index, record) in self.records.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                NominalInterfaceSetSemanticValidationError::Record { index, error }
            })?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalNominalInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalInterfacesV1 {
    records: Vec<DecodedNominalInterfaceRecordV1>,
}

impl DecodedCanonicalNominalInterfacesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalNominalInterfacesV1, NominalInterfaceSetValidationError<E>>
    where
        R: NominalInterfaceRecordResolver<E>,
    {
        let mut records = Vec::<NominalInterfaceRecordV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record
                .resolve(resolver)
                .map_err(|error| NominalInterfaceSetValidationError::Record { index, error })?;
            if let Some(previous) = records.last() {
                match previous.declaration().cmp(&record.declaration()) {
                    std::cmp::Ordering::Equal => {
                        return Err(NominalInterfaceSetValidationError::DuplicateDeclaration {
                            index,
                            declaration: record.declaration(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(NominalInterfaceSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalNominalInterfacesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalNominalInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalNominalInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedNominalInterfaceRecordV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalInterfaceSetBuildError {
    DuplicateDeclaration(SourceNominalId),
}

impl fmt::Display for NominalInterfaceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDeclaration(declaration) => {
                write!(formatter, "duplicate nominal interface {declaration:?}")
            }
        }
    }
}

impl std::error::Error for NominalInterfaceSetBuildError {}

#[derive(Debug)]
pub enum NominalInterfaceSetValidationError<E> {
    Record {
        index: usize,
        error: NominalInterfaceRecordResolutionError<E>,
    },
    DuplicateDeclaration {
        index: usize,
        declaration: SourceNominalId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for NominalInterfaceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(formatter, "invalid nominal interface {index}: {error}")
            }
            Self::DuplicateDeclaration { index, declaration } => write!(
                formatter,
                "duplicate nominal interface {declaration:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => {
                write!(
                    formatter,
                    "non-canonical nominal interface order at index {index}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for NominalInterfaceSetValidationError<E> {}

#[derive(Debug)]
pub enum NominalInterfaceSetSemanticValidationError<E> {
    Record {
        index: usize,
        error: super::NominalInterfaceSemanticValidationError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for NominalInterfaceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => write!(
                formatter,
                "invalid nominal interface semantics at index {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for NominalInterfaceSetSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
