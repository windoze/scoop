use std::fmt;

use scoop_identity::{
    ConeIdentity, PersistentIdResolver, PersistentKeyResolver, PersistentSourceContextId,
    SourceContextKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CallableSourceInterfaceIndexError, CallableSourceInterfaceResolutionError,
    CallableSourceInterfaceV1, DecodedCallableSourceInterfaceV1,
    ExportDefaultTemplateIndexResolver, ExportDefaultTemplateKeyResolver,
    IndexedCallableSourceInterfaceV1,
};
use crate::{CallableDeclarationId, CallableDeclarationIdResolver, SignatureTypeReferenceResolver};

mod semantics;

pub use semantics::CallableSourceInterfaceSetSemanticValidationError;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalCallableSourceInterfacesV1 {
    records: Vec<CallableSourceInterfaceV1>,
}

impl CanonicalCallableSourceInterfacesV1 {
    pub fn try_new(
        mut records: Vec<CallableSourceInterfaceV1>,
    ) -> Result<Self, CallableSourceInterfaceSetBuildError> {
        records.sort_unstable_by_key(CallableSourceInterfaceV1::owner);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].owner() == pair[1].owner())
        {
            return Err(CallableSourceInterfaceSetBuildError::DuplicateOwner(
                pair[0].owner(),
            ));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[CallableSourceInterfaceV1] {
        &self.records
    }

    pub fn get(&self, owner: CallableDeclarationId) -> Option<&CallableSourceInterfaceV1> {
        self.records
            .binary_search_by_key(&owner, CallableSourceInterfaceV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn index_templates<I>(
        &self,
        resolver: &mut I,
    ) -> Result<
        IndexedCanonicalCallableSourceInterfacesV1<'_>,
        CallableSourceInterfaceSetIndexError<I::Error>,
    >
    where
        I: ExportDefaultTemplateIndexResolver,
    {
        let mut records = Vec::with_capacity(self.records.len());
        for (index, record) in self.records.iter().enumerate() {
            records.push(
                record.index_templates(resolver).map_err(|error| {
                    CallableSourceInterfaceSetIndexError::Record { index, error }
                })?,
            );
        }
        Ok(IndexedCanonicalCallableSourceInterfacesV1 { records })
    }
}

pub struct IndexedCanonicalCallableSourceInterfacesV1<'a> {
    records: Vec<IndexedCallableSourceInterfaceV1<'a>>,
}

impl WireEncode for IndexedCanonicalCallableSourceInterfacesV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalCallableSourceInterfacesV1 {
    records: Vec<DecodedCallableSourceInterfaceV1>,
}

impl DecodedCanonicalCallableSourceInterfacesV1 {
    pub fn resolve<R, T, E>(
        self,
        resolver: &mut R,
        templates: &mut T,
    ) -> Result<
        CanonicalCallableSourceInterfacesV1,
        CallableSourceInterfaceSetValidationError<E, T::Error>,
    >
    where
        R: CallableDeclarationIdResolver<E>
            + SignatureTypeReferenceResolver<E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
        T: ExportDefaultTemplateKeyResolver,
    {
        let mut records = Vec::<CallableSourceInterfaceV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record.resolve(resolver, templates).map_err(|error| {
                CallableSourceInterfaceSetValidationError::Record { index, error }
            })?;
            if let Some(previous) = records.last() {
                match previous.owner().cmp(&record.owner()) {
                    std::cmp::Ordering::Equal => {
                        return Err(CallableSourceInterfaceSetValidationError::DuplicateOwner {
                            index,
                            owner: record.owner(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(
                            CallableSourceInterfaceSetValidationError::NonCanonicalOrder { index },
                        );
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalCallableSourceInterfacesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalCallableSourceInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalCallableSourceInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCallableSourceInterfaceV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableSourceInterfaceSetBuildError {
    DuplicateOwner(CallableDeclarationId),
}

impl fmt::Display for CallableSourceInterfaceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateOwner(owner) => {
                write!(formatter, "duplicate callable source interface {owner:?}")
            }
        }
    }
}

impl std::error::Error for CallableSourceInterfaceSetBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableSourceInterfaceSetValidationError<E, T> {
    Record {
        index: usize,
        error: CallableSourceInterfaceResolutionError<E, T>,
    },
    DuplicateOwner {
        index: usize,
        owner: CallableDeclarationId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display, T: fmt::Display> fmt::Display
    for CallableSourceInterfaceSetValidationError<E, T>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "invalid callable source interface {index}: {error}"
                )
            }
            Self::DuplicateOwner { index, owner } => write!(
                formatter,
                "duplicate callable source interface {owner:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical callable source interface order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static, T: std::error::Error + 'static> std::error::Error
    for CallableSourceInterfaceSetValidationError<E, T>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableSourceInterfaceSetIndexError<E> {
    Record {
        index: usize,
        error: CallableSourceInterfaceIndexError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for CallableSourceInterfaceSetIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "cannot index callable source interface {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallableSourceInterfaceSetIndexError<E> {}

#[cfg(test)]
mod tests;
