use super::{
    CallableDeclarationRecordV1, CallableInterfaceRecordResolutionError,
    CallableInterfaceRecordResolver, CallableInterfaceRecordV1, CallableInterfaceSemanticAuthority,
    CallableInterfaceSemanticValidationError, DecodedCallableDeclarationRecordV1,
    DecodedCallableInterfaceRecordV1,
};
use crate::CallableDeclarationId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};
use std::fmt;

mod decode;
mod errors;
pub use decode::DecodedCanonicalCallableInterfacesV1;
pub use errors::{
    CallableInterfaceSetBuildError, CallableInterfaceSetSemanticValidationError,
    CallableInterfaceSetValidationError,
};

/// Public lookup and necessary source support share one declaration per typed id.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalCallableInterfacesV1 {
    records: Vec<CallableInterfaceRecordV1>,
    support: Vec<CallableDeclarationRecordV1>,
}

impl CanonicalCallableInterfacesV1 {
    pub fn try_new(
        records: Vec<CallableInterfaceRecordV1>,
    ) -> Result<Self, CallableInterfaceSetBuildError> {
        Self::with_support(records, Vec::new())
    }

    pub fn with_support(
        mut records: Vec<CallableInterfaceRecordV1>,
        mut support: Vec<CallableDeclarationRecordV1>,
    ) -> Result<Self, CallableInterfaceSetBuildError> {
        records.sort_unstable_by_key(CallableInterfaceRecordV1::declaration);
        support.sort_unstable_by_key(CallableDeclarationRecordV1::declaration);
        let duplicate = records
            .windows(2)
            .find(|pair| pair[0].declaration() == pair[1].declaration())
            .map(|pair| pair[0].declaration())
            .or_else(|| {
                support
                    .windows(2)
                    .find(|pair| pair[0].declaration() == pair[1].declaration())
                    .map(|pair| pair[0].declaration())
            });
        if let Some(declaration) = duplicate {
            return Err(CallableInterfaceSetBuildError::DuplicateDeclaration(
                declaration,
            ));
        }
        for record in &support {
            if records
                .binary_search_by_key(
                    &record.declaration(),
                    CallableInterfaceRecordV1::declaration,
                )
                .is_ok()
            {
                return Err(CallableInterfaceSetBuildError::DuplicateDeclaration(
                    record.declaration(),
                ));
            }
        }
        Ok(Self { records, support })
    }

    pub fn records(&self) -> &[CallableInterfaceRecordV1] {
        &self.records
    }
    pub fn support_records(&self) -> &[CallableDeclarationRecordV1] {
        &self.support
    }
    pub fn all_declarations(&self) -> impl Iterator<Item = &CallableDeclarationRecordV1> {
        self.records
            .iter()
            .map(CallableInterfaceRecordV1::declaration_data)
            .chain(&self.support)
    }
    pub(crate) fn wire_declarations(
        &self,
        path: scoop_wire::WirePath,
    ) -> impl Iterator<Item = (scoop_wire::WirePath, &CallableDeclarationRecordV1)> {
        let public = path.clone().field(1);
        let support = path.field(2);
        self.records
            .iter()
            .enumerate()
            .map(move |(index, record)| {
                (
                    public.clone().index(index as u64).field(1),
                    record.declaration_data(),
                )
            })
            .chain(
                self.support
                    .iter()
                    .enumerate()
                    .map(move |(index, record)| (support.clone().index(index as u64), record)),
            )
    }
    pub fn declaration_count(&self) -> usize {
        self.records.len() + self.support.len()
    }
    pub fn get(&self, declaration: CallableDeclarationId) -> Option<&CallableInterfaceRecordV1> {
        self.records
            .binary_search_by_key(&declaration, CallableInterfaceRecordV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }
    pub fn declaration(
        &self,
        declaration: CallableDeclarationId,
    ) -> Option<&CallableDeclarationRecordV1> {
        self.get(declaration)
            .map(CallableInterfaceRecordV1::declaration_data)
            .or_else(|| {
                self.support
                    .binary_search_by_key(&declaration, CallableDeclarationRecordV1::declaration)
                    .ok()
                    .map(|index| &self.support[index])
            })
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.support.is_empty()
    }

    pub fn validate_semantics<A: CallableInterfaceSemanticAuthority<E>, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), CallableInterfaceSetSemanticValidationError<E>> {
        for (index, record) in self.records.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                CallableInterfaceSetSemanticValidationError::Record { index, error }
            })?;
        }
        Ok(())
    }

    pub fn validate_support_semantics<A: CallableInterfaceSemanticAuthority<E>, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), CallableInterfaceSetSemanticValidationError<E>> {
        for (index, record) in self.support.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                CallableInterfaceSetSemanticValidationError::SupportRecord { index, error }
            })?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalCallableInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        encoder.field(2)?;
        encoder.array(self.support.len() as u64)?;
        for record in &self.support {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
