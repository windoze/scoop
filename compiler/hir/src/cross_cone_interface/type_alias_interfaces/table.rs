use std::fmt;

use scoop_identity::PersistentTypeAliasId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedTypeAliasInterfaceRecordV1, TypeAliasInterfaceRecordResolutionError,
    TypeAliasInterfaceRecordResolver, TypeAliasInterfaceRecordV1,
    TypeAliasInterfaceSemanticAuthority, TypeAliasInterfaceSemanticValidationError,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalTypeAliasInterfacesV1 {
    records: Vec<TypeAliasInterfaceRecordV1>,
}

impl CanonicalTypeAliasInterfacesV1 {
    pub fn try_new(
        mut records: Vec<TypeAliasInterfaceRecordV1>,
    ) -> Result<Self, TypeAliasInterfaceSetBuildError> {
        records.sort_unstable_by_key(TypeAliasInterfaceRecordV1::alias);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].alias() == pair[1].alias())
        {
            return Err(TypeAliasInterfaceSetBuildError::DuplicateAlias(
                pair[0].alias(),
            ));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[TypeAliasInterfaceRecordV1] {
        &self.records
    }

    pub fn get(&self, alias: PersistentTypeAliasId) -> Option<&TypeAliasInterfaceRecordV1> {
        self.records
            .binary_search_by_key(&alias, TypeAliasInterfaceRecordV1::alias)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), TypeAliasInterfaceSetSemanticValidationError<E>>
    where
        A: TypeAliasInterfaceSemanticAuthority<E>,
    {
        for (index, record) in self.records.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                TypeAliasInterfaceSetSemanticValidationError::Record { index, error }
            })?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalTypeAliasInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalTypeAliasInterfacesV1 {
    records: Vec<DecodedTypeAliasInterfaceRecordV1>,
}

impl DecodedCanonicalTypeAliasInterfacesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalTypeAliasInterfacesV1, TypeAliasInterfaceSetValidationError<E>>
    where
        R: TypeAliasInterfaceRecordResolver<E>,
    {
        let mut records = Vec::<TypeAliasInterfaceRecordV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record
                .resolve(resolver)
                .map_err(|error| TypeAliasInterfaceSetValidationError::Record { index, error })?;
            if let Some(previous) = records.last() {
                match previous.alias().cmp(&record.alias()) {
                    std::cmp::Ordering::Equal => {
                        return Err(TypeAliasInterfaceSetValidationError::DuplicateAlias {
                            index,
                            alias: record.alias(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(TypeAliasInterfaceSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalTypeAliasInterfacesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalTypeAliasInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalTypeAliasInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedTypeAliasInterfaceRecordV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeAliasInterfaceSetBuildError {
    DuplicateAlias(PersistentTypeAliasId),
}

impl fmt::Display for TypeAliasInterfaceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAlias(alias) => {
                write!(formatter, "duplicate type-alias interface {alias}")
            }
        }
    }
}

impl std::error::Error for TypeAliasInterfaceSetBuildError {}

#[derive(Debug)]
pub enum TypeAliasInterfaceSetValidationError<E> {
    Record {
        index: usize,
        error: TypeAliasInterfaceRecordResolutionError<E>,
    },
    DuplicateAlias {
        index: usize,
        alias: PersistentTypeAliasId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for TypeAliasInterfaceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(formatter, "invalid type-alias interface {index}: {error}")
            }
            Self::DuplicateAlias { index, alias } => write!(
                formatter,
                "duplicate type-alias interface {alias} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical type-alias interface order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TypeAliasInterfaceSetValidationError<E> {}

#[derive(Debug)]
pub enum TypeAliasInterfaceSetSemanticValidationError<E> {
    Record {
        index: usize,
        error: TypeAliasInterfaceSemanticValidationError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for TypeAliasInterfaceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(formatter, "invalid type-alias interface {index}: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for TypeAliasInterfaceSetSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
