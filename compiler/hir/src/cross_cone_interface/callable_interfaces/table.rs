use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CallableInterfaceRecordResolutionError, CallableInterfaceRecordResolver,
    CallableInterfaceRecordV1, CallableInterfaceSemanticAuthority,
    CallableInterfaceSemanticValidationError, DecodedCallableInterfaceRecordV1,
};
use crate::CallableDeclarationId;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalCallableInterfacesV1 {
    records: Vec<CallableInterfaceRecordV1>,
}

impl CanonicalCallableInterfacesV1 {
    pub fn try_new(
        mut records: Vec<CallableInterfaceRecordV1>,
    ) -> Result<Self, CallableInterfaceSetBuildError> {
        records.sort_unstable_by_key(CallableInterfaceRecordV1::declaration);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].declaration() == pair[1].declaration())
        {
            return Err(CallableInterfaceSetBuildError::DuplicateDeclaration(
                pair[0].declaration(),
            ));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[CallableInterfaceRecordV1] {
        &self.records
    }

    pub fn get(&self, declaration: CallableDeclarationId) -> Option<&CallableInterfaceRecordV1> {
        self.records
            .binary_search_by_key(&declaration, CallableInterfaceRecordV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), CallableInterfaceSetSemanticValidationError<E>>
    where
        A: CallableInterfaceSemanticAuthority<E>,
    {
        for (index, record) in self.records.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                CallableInterfaceSetSemanticValidationError::Record { index, error }
            })?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalCallableInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalCallableInterfacesV1 {
    records: Vec<DecodedCallableInterfaceRecordV1>,
}

impl DecodedCanonicalCallableInterfacesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCallableInterfacesV1, CallableInterfaceSetValidationError<E>>
    where
        R: CallableInterfaceRecordResolver<E>,
    {
        let mut records = Vec::<CallableInterfaceRecordV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record
                .resolve(resolver)
                .map_err(|error| CallableInterfaceSetValidationError::Record { index, error })?;
            if let Some(previous) = records.last() {
                match previous.declaration().cmp(&record.declaration()) {
                    std::cmp::Ordering::Equal => {
                        return Err(CallableInterfaceSetValidationError::DuplicateDeclaration {
                            index,
                            declaration: record.declaration(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(CallableInterfaceSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalCallableInterfacesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalCallableInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalCallableInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCallableInterfaceRecordV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableInterfaceSetBuildError {
    DuplicateDeclaration(CallableDeclarationId),
}

impl fmt::Display for CallableInterfaceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDeclaration(declaration) => {
                write!(formatter, "duplicate callable interface {declaration:?}")
            }
        }
    }
}

impl std::error::Error for CallableInterfaceSetBuildError {}

#[derive(Debug)]
pub enum CallableInterfaceSetValidationError<E> {
    Record {
        index: usize,
        error: CallableInterfaceRecordResolutionError<E>,
    },
    DuplicateDeclaration {
        index: usize,
        declaration: CallableDeclarationId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for CallableInterfaceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(formatter, "invalid callable interface {index}: {error}")
            }
            Self::DuplicateDeclaration { index, declaration } => write!(
                formatter,
                "duplicate callable interface {declaration:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical callable interface order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallableInterfaceSetValidationError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableInterfaceSetSemanticValidationError<E> {
    Record {
        index: usize,
        error: CallableInterfaceSemanticValidationError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for CallableInterfaceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "invalid callable interface semantics {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CallableInterfaceSetSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
