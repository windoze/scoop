use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedPropertyInterfaceRecordV1, PropertyInterfaceRecordResolutionError,
    PropertyInterfaceRecordResolver, PropertyInterfaceRecordV1, PropertyInterfaceSemanticAuthority,
    PropertyInterfaceSemanticValidationError,
};
use crate::PropertyDeclarationId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPropertyInterfacesV1 {
    records: Vec<PropertyInterfaceRecordV1>,
}

impl CanonicalPropertyInterfacesV1 {
    pub fn try_new(
        mut records: Vec<PropertyInterfaceRecordV1>,
    ) -> Result<Self, PropertyInterfaceSetBuildError> {
        records.sort_unstable_by_key(PropertyInterfaceRecordV1::declaration);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].declaration() == pair[1].declaration())
        {
            return Err(PropertyInterfaceSetBuildError::DuplicateDeclaration(
                pair[0].declaration(),
            ));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[PropertyInterfaceRecordV1] {
        &self.records
    }

    pub fn get(&self, declaration: PropertyDeclarationId) -> Option<&PropertyInterfaceRecordV1> {
        self.records
            .binary_search_by_key(&declaration, PropertyInterfaceRecordV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), PropertyInterfaceSetSemanticValidationError<E>>
    where
        A: PropertyInterfaceSemanticAuthority<E>,
    {
        for (index, record) in self.records.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                PropertyInterfaceSetSemanticValidationError::Record { index, error }
            })?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalPropertyInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalPropertyInterfacesV1 {
    records: Vec<DecodedPropertyInterfaceRecordV1>,
}

impl DecodedCanonicalPropertyInterfacesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalPropertyInterfacesV1, PropertyInterfaceSetValidationError<E>>
    where
        R: PropertyInterfaceRecordResolver<E>,
    {
        let mut records = Vec::<PropertyInterfaceRecordV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record
                .resolve(resolver)
                .map_err(|error| PropertyInterfaceSetValidationError::Record { index, error })?;
            if let Some(previous) = records.last() {
                match previous.declaration().cmp(&record.declaration()) {
                    std::cmp::Ordering::Equal => {
                        return Err(PropertyInterfaceSetValidationError::DuplicateDeclaration {
                            index,
                            declaration: record.declaration(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(PropertyInterfaceSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalPropertyInterfacesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalPropertyInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalPropertyInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedPropertyInterfaceRecordV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyInterfaceSetBuildError {
    DuplicateDeclaration(PropertyDeclarationId),
}

impl fmt::Display for PropertyInterfaceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDeclaration(declaration) => {
                write!(formatter, "duplicate property interface {declaration:?}")
            }
        }
    }
}

impl std::error::Error for PropertyInterfaceSetBuildError {}

#[derive(Debug)]
pub enum PropertyInterfaceSetValidationError<E> {
    Record {
        index: usize,
        error: PropertyInterfaceRecordResolutionError<E>,
    },
    DuplicateDeclaration {
        index: usize,
        declaration: PropertyDeclarationId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for PropertyInterfaceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(formatter, "invalid property interface {index}: {error}")
            }
            Self::DuplicateDeclaration { index, declaration } => write!(
                formatter,
                "duplicate property interface {declaration:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical property interface order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PropertyInterfaceSetValidationError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum PropertyInterfaceSetSemanticValidationError<E> {
    Record {
        index: usize,
        error: PropertyInterfaceSemanticValidationError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for PropertyInterfaceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => write!(
                formatter,
                "invalid property interface semantics {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for PropertyInterfaceSetSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
