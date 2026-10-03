use std::fmt;

mod errors;
pub use errors::*;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedNominalInterfaceRecordV1, NominalInterfaceRecordResolutionError,
    NominalInterfaceRecordResolver, NominalInterfaceRecordV1, SourceNominalId,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalInterfacesV1 {
    records: Vec<NominalInterfaceRecordV1>,
    support: Vec<NominalInterfaceRecordV1>,
}

impl CanonicalNominalInterfacesV1 {
    pub fn try_new(
        records: Vec<NominalInterfaceRecordV1>,
    ) -> Result<Self, NominalInterfaceSetBuildError> {
        Self::with_support(records, Vec::new())
    }

    pub fn with_support(
        mut records: Vec<NominalInterfaceRecordV1>,
        mut support: Vec<NominalInterfaceRecordV1>,
    ) -> Result<Self, NominalInterfaceSetBuildError> {
        records.sort_unstable_by_key(NominalInterfaceRecordV1::declaration);
        support.sort_unstable_by_key(NominalInterfaceRecordV1::declaration);
        for values in [&records, &support] {
            if let Some(pair) = values
                .windows(2)
                .find(|pair| pair[0].declaration() == pair[1].declaration())
            {
                return Err(NominalInterfaceSetBuildError::DuplicateDeclaration(
                    pair[0].declaration(),
                ));
            }
        }
        let table = Self { records, support };
        table.validate_partition()?;
        Ok(table)
    }

    fn validate_partition(&self) -> Result<(), NominalInterfaceSetBuildError> {
        for record in &self.records {
            if record.declaration_details().declared_visibility()
                != crate::DeclaredVisibilityV1::Public
            {
                return Err(NominalInterfaceSetBuildError::NonPublicDeclaration(
                    record.declaration(),
                ));
            }
        }
        for record in &self.support {
            if self.get(record.declaration()).is_some() {
                return Err(NominalInterfaceSetBuildError::DuplicateDeclaration(
                    record.declaration(),
                ));
            }
            if !record.constructors().is_empty()
                || !record.members().members().is_empty()
                || !record.nested_bindings().is_empty()
            {
                return Err(NominalInterfaceSetBuildError::SupportLookup(
                    record.declaration(),
                ));
            }
        }
        Ok(())
    }

    pub fn records(&self) -> &[NominalInterfaceRecordV1] {
        &self.records
    }

    pub fn support_records(&self) -> &[NominalInterfaceRecordV1] {
        &self.support
    }

    pub fn all_records(&self) -> impl Iterator<Item = &NominalInterfaceRecordV1> {
        self.records.iter().chain(&self.support)
    }

    pub(crate) fn wire_records(
        &self,
    ) -> impl Iterator<Item = (u32, usize, &NominalInterfaceRecordV1)> {
        self.records
            .iter()
            .enumerate()
            .map(|(index, record)| (1, index, record))
            .chain(
                self.support
                    .iter()
                    .enumerate()
                    .map(|(index, record)| (2, index, record)),
            )
    }

    pub fn declaration_count(&self) -> usize {
        self.records.len() + self.support.len()
    }

    /// Source lookup includes necessary support and grants no public binding.
    pub fn declaration(&self, declaration: SourceNominalId) -> Option<&NominalInterfaceRecordV1> {
        self.get(declaration).or_else(|| {
            self.support
                .binary_search_by_key(&declaration, NominalInterfaceRecordV1::declaration)
                .ok()
                .map(|index| &self.support[index])
        })
    }

    pub fn get(&self, declaration: SourceNominalId) -> Option<&NominalInterfaceRecordV1> {
        self.records
            .binary_search_by_key(&declaration, NominalInterfaceRecordV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.support.is_empty()
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

    /// Support signatures use source-declaration queries, independently of
    /// the public-signature visibility checks above.
    pub fn validate_support_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), NominalInterfaceSetSemanticValidationError<E>>
    where
        A: super::NominalInterfaceSemanticAuthority<E>,
    {
        for (index, record) in self.support.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                NominalInterfaceSetSemanticValidationError::Record { index, error }
            })?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalNominalInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        for (field, records) in [(1, &self.records), (2, &self.support)] {
            encoder.field(field)?;
            encoder.array(records.len() as u64)?;
            for record in records {
                record.encode(encoder)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalInterfacesV1 {
    records: Vec<DecodedNominalInterfaceRecordV1>,
    support: Vec<DecodedNominalInterfaceRecordV1>,
}

impl DecodedCanonicalNominalInterfacesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalNominalInterfacesV1, NominalInterfaceSetValidationError<E>>
    where
        R: NominalInterfaceRecordResolver<E>,
    {
        let records = Self::resolve_records(self.records, resolver)?;
        let support = Self::resolve_records(self.support, resolver)?;
        let table = CanonicalNominalInterfacesV1 { records, support };
        table
            .validate_partition()
            .map_err(NominalInterfaceSetValidationError::Partition)?;
        Ok(table)
    }

    fn resolve_records<R: NominalInterfaceRecordResolver<E>, E>(
        decoded: Vec<DecodedNominalInterfaceRecordV1>,
        resolver: &mut R,
    ) -> Result<Vec<NominalInterfaceRecordV1>, NominalInterfaceSetValidationError<E>> {
        let mut records = Vec::<NominalInterfaceRecordV1>::with_capacity(decoded.len());
        for (index, record) in decoded.into_iter().enumerate() {
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
        Ok(records)
    }
}

impl WireEncode for DecodedCanonicalNominalInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        for (field, records) in [(1, &self.records), (2, &self.support)] {
            encoder.field(field)?;
            encoder.array(records.len() as u64)?;
            for record in records {
                record.encode(encoder)?;
            }
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalNominalInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let value = Self {
            records: decoder.field(1, |d| {
                d.decode_array(|d, _| DecodedNominalInterfaceRecordV1::decode(d))
            })?,
            support: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedNominalInterfaceRecordV1::decode(d))
            })?,
        };

        Ok(value)
    }
}

#[cfg(test)]
mod tests;
