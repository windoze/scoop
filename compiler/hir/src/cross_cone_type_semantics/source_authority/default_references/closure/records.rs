use super::*;
use DefaultSourceReferenceClosureError as Error;
use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
pub enum DefaultSourceReferenceRecordV1<'a> {
    Callable(&'a DefaultSourceReferenceV1<ExportDefaultCallableTargetV1>),
    Constructor(&'a DefaultSourceReferenceV1<DefaultConstructorRefV1>),
    Type(&'a DefaultSourceReferenceV1<SignatureTypeKey>),
    Global(&'a DefaultSourceReferenceV1<PersistentPropertyId>),
    Singleton(&'a DefaultSourceReferenceV1<PersistentObjectValueId>),
    Field(&'a DefaultSourceReferenceV1<DefaultFieldRefV1>),
}
impl<'a> DefaultSourceReferenceRecordV1<'a> {
    pub const fn kind(self) -> ExportDefaultReferenceKindV1 {
        match self {
            Self::Callable(_) => ExportDefaultReferenceKindV1::Callable,
            Self::Constructor(_) => ExportDefaultReferenceKindV1::Constructor,
            Self::Type(_) => ExportDefaultReferenceKindV1::Type,
            Self::Global(_) => ExportDefaultReferenceKindV1::Global,
            Self::Singleton(_) => ExportDefaultReferenceKindV1::Singleton,
            Self::Field(_) => ExportDefaultReferenceKindV1::Field,
        }
    }
    pub const fn witness(self) -> &'a DefaultSourceAccessWitnessV1 {
        match self {
            Self::Callable(r) => r.witness(),
            Self::Constructor(r) => r.witness(),
            Self::Type(r) => r.witness(),
            Self::Global(r) => r.witness(),
            Self::Singleton(r) => r.witness(),
            Self::Field(r) => r.witness(),
        }
    }
    pub const fn definition_origin(self) -> &'a ExportDefinitionSourceV1 {
        match self {
            Self::Callable(r) => r.definition_origin(),
            Self::Constructor(r) => r.definition_origin(),
            Self::Type(r) => r.definition_origin(),
            Self::Global(r) => r.definition_origin(),
            Self::Singleton(r) => r.definition_origin(),
            Self::Field(r) => r.definition_origin(),
        }
    }
}
pub(super) struct Domain<'a, T> {
    pub records: &'a [DefaultSourceReferenceV1<T>],
    pub kind: ExportDefaultReferenceKindV1,
    next: usize,
}
impl<'a, T> Domain<'a, T> {
    pub fn new(
        records: &'a [DefaultSourceReferenceV1<T>],
        kind: ExportDefaultReferenceKindV1,
    ) -> Self {
        Self {
            records,
            kind,
            next: 0,
        }
    }
    pub fn observe(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
        compare: impl FnOnce(&T, &WirePath) -> Result<Ordering, WireError>,

        path: &WirePath,
    ) -> Result<(u32, &'a DefaultSourceReferenceV1<T>), Error> {
        let kind = self.kind;
        let index = self.next as u32;
        let site = occurrence.site;
        let record = self
            .records
            .get(self.next)
            .ok_or(Error::Missing { kind, index, site })?;
        if !compare(record.target(), path)?.is_eq() {
            return Err(Error::Target { kind, index, site });
        }

        if occurrence.definition_origin != record.definition_origin() {
            return Err(Error::Origin { kind, index, site });
        }
        self.next += 1;
        Ok((index, record))
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.next != self.records.len() {
            return Err(Error::Extra {
                kind: self.kind,
                index: self.next as u32,
            });
        }
        Ok(())
    }
}
