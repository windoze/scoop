use super::{
    DecodedPropertyDeclarationRecordV1, DecodedPropertyInterfaceRecordV1,
    PropertyDeclarationRecordV1, PropertyInterfaceRecordResolutionError,
    PropertyInterfaceRecordResolver, PropertyInterfaceRecordV1, PropertyInterfaceSemanticAuthority,
    PropertyInterfaceSemanticValidationError,
};
use crate::PropertyDeclarationId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};
use std::fmt;

mod decode;
mod errors;
pub use decode::DecodedCanonicalPropertyInterfacesV1;
pub use errors::{
    PropertyInterfaceSetBuildError, PropertyInterfaceSetSemanticValidationError,
    PropertyInterfaceSetValidationError,
};

/// Public lookup and necessary source support share one declaration per typed id.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalPropertyInterfacesV1 {
    records: Vec<PropertyInterfaceRecordV1>,
    support: Vec<PropertyDeclarationRecordV1>,
}

impl CanonicalPropertyInterfacesV1 {
    pub fn try_new(
        records: Vec<PropertyInterfaceRecordV1>,
    ) -> Result<Self, PropertyInterfaceSetBuildError> {
        Self::with_support(records, Vec::new())
    }

    pub fn with_support(
        mut records: Vec<PropertyInterfaceRecordV1>,
        mut support: Vec<PropertyDeclarationRecordV1>,
    ) -> Result<Self, PropertyInterfaceSetBuildError> {
        records.sort_unstable_by_key(PropertyInterfaceRecordV1::declaration);
        support.sort_unstable_by_key(PropertyDeclarationRecordV1::declaration);
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
            return Err(PropertyInterfaceSetBuildError::DuplicateDeclaration(
                declaration,
            ));
        }
        for record in &support {
            if records
                .binary_search_by_key(
                    &record.declaration(),
                    PropertyInterfaceRecordV1::declaration,
                )
                .is_ok()
            {
                return Err(PropertyInterfaceSetBuildError::DuplicateDeclaration(
                    record.declaration(),
                ));
            }
        }
        Ok(Self { records, support })
    }

    pub fn records(&self) -> &[PropertyInterfaceRecordV1] {
        &self.records
    }
    pub fn support_records(&self) -> &[PropertyDeclarationRecordV1] {
        &self.support
    }
    pub fn all_declarations(&self) -> impl Iterator<Item = &PropertyDeclarationRecordV1> {
        self.records
            .iter()
            .map(PropertyInterfaceRecordV1::declaration_data)
            .chain(&self.support)
    }
    pub fn declaration_count(&self) -> usize {
        self.records.len() + self.support.len()
    }
    pub fn get(&self, declaration: PropertyDeclarationId) -> Option<&PropertyInterfaceRecordV1> {
        self.records
            .binary_search_by_key(&declaration, PropertyInterfaceRecordV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }
    pub fn declaration(
        &self,
        declaration: PropertyDeclarationId,
    ) -> Option<&PropertyDeclarationRecordV1> {
        self.get(declaration)
            .map(PropertyInterfaceRecordV1::declaration_data)
            .or_else(|| {
                self.support
                    .binary_search_by_key(&declaration, PropertyDeclarationRecordV1::declaration)
                    .ok()
                    .map(|index| &self.support[index])
            })
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.support.is_empty()
    }

    pub fn validate_semantics<A: PropertyInterfaceSemanticAuthority<E>, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), PropertyInterfaceSetSemanticValidationError<E>> {
        for (index, record) in self.records.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                PropertyInterfaceSetSemanticValidationError::Record { index, error }
            })?;
        }
        Ok(())
    }

    pub fn validate_support_semantics<A: PropertyInterfaceSemanticAuthority<E>, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), PropertyInterfaceSetSemanticValidationError<E>> {
        for (index, record) in self.support.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                PropertyInterfaceSetSemanticValidationError::SupportRecord { index, error }
            })?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalPropertyInterfacesV1 {
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
