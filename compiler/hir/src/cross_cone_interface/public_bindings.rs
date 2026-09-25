use std::fmt;

use scoop_identity::{
    BindableEntity, BindingIdentityResolutionError, BindingResolver, DecodedBindableEntity,
    DecodedPersistentId, ExportBindingKey, PersistentExportBindingId, PersistentIdResolver,
    PersistentKeyResolver,
};
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath,
};

use crate::{CanonicalReexportRoutesV1, DecodedCanonicalReexportRoutesV1};

mod direct_surface;

pub use direct_surface::PublicExportBindingDirectSurfaceValidationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportBindingSourceV1 {
    DeclaredCurrent { declaration: BindableEntity },
    Reexport { routes: CanonicalReexportRoutesV1 },
}

impl WireEncode for ExportBindingSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::DeclaredCurrent { .. } => 1,
            Self::Reexport { .. } => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::DeclaredCurrent { declaration } => declaration.encode(encoder),
            Self::Reexport { routes } => routes.encode(encoder),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicExportBindingRecordV1 {
    binding: PersistentExportBindingId,
    source: ExportBindingSourceV1,
}

impl PublicExportBindingRecordV1 {
    pub const fn new(binding: PersistentExportBindingId, source: ExportBindingSourceV1) -> Self {
        Self { binding, source }
    }

    pub const fn binding(&self) -> PersistentExportBindingId {
        self.binding
    }

    pub const fn source(&self) -> &ExportBindingSourceV1 {
        &self.source
    }
}

impl WireEncode for PublicExportBindingRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.source.encode(encoder)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalPublicExportBindingsV1 {
    records: Vec<PublicExportBindingRecordV1>,
}

impl CanonicalPublicExportBindingsV1 {
    pub fn try_new(
        mut records: Vec<PublicExportBindingRecordV1>,
    ) -> Result<Self, PublicExportBindingBuildError> {
        records.sort_unstable_by_key(PublicExportBindingRecordV1::binding);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].binding == pair[1].binding)
        {
            return Err(PublicExportBindingBuildError::DuplicateBinding(
                pair[0].binding,
            ));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[PublicExportBindingRecordV1] {
        &self.records
    }

    pub fn get(&self, binding: PersistentExportBindingId) -> Option<&PublicExportBindingRecordV1> {
        self.records
            .binary_search_by_key(&binding, PublicExportBindingRecordV1::binding)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn get_metered(
        &self,
        binding: PersistentExportBindingId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<&PublicExportBindingRecordV1>, WireError> {
        meter.check_table_entries(self.records.len() as u64, path)?;
        let mut start = 0;
        let mut end = self.records.len();
        while start < end {
            meter.charge_work(1, path)?;
            let middle = start + (end - start) / 2;
            match self.records[middle].binding().cmp(&binding) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Ok(Some(&self.records[middle])),
            }
        }
        Ok(None)
    }
}

impl WireEncode for CanonicalPublicExportBindingsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedExportBindingSourceV1 {
    DeclaredCurrent {
        declaration: DecodedBindableEntity,
    },
    Reexport {
        routes: DecodedCanonicalReexportRoutesV1,
    },
}

impl WireEncode for DecodedExportBindingSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::DeclaredCurrent { .. } => 1,
            Self::Reexport { .. } => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::DeclaredCurrent { declaration } => declaration.encode(encoder),
            Self::Reexport { routes } => routes.encode(encoder),
        }
    }
}

impl WireDecode for DecodedExportBindingSourceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        if fields != 2 {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        match tag {
            1 => decoder
                .field(1, DecodedBindableEntity::decode)
                .map(|declaration| Self::DeclaredCurrent { declaration }),
            2 => decoder
                .field(1, DecodedCanonicalReexportRoutesV1::decode)
                .map(|routes| Self::Reexport { routes }),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPublicExportBindingRecordV1 {
    binding: DecodedPersistentId<PersistentExportBindingId>,
    source: DecodedExportBindingSourceV1,
}

impl DecodedPublicExportBindingRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PublicExportBindingRecordV1, PublicExportBindingResolutionError<E>>
    where
        R: PublicExportBindingResolver<E>,
    {
        let binding =
            <R as PersistentIdResolver<PersistentExportBindingId>>::resolve(resolver, self.binding)
                .map_err(PublicExportBindingResolutionError::Binding)?;
        let key =
            <R as PersistentKeyResolver<PersistentExportBindingId, ExportBindingKey>>::resolve_key(
                resolver,
                self.binding,
            )
            .map_err(PublicExportBindingResolutionError::BindingKey)?;
        let source = match self.source {
            DecodedExportBindingSourceV1::DeclaredCurrent { declaration } => {
                let declaration = declaration
                    .resolve_target(key.namespace(), key.role(), resolver)
                    .map_err(PublicExportBindingResolutionError::Declaration)?
                    .target();
                if declaration != key.target() {
                    return Err(PublicExportBindingResolutionError::DeclarationMismatch {
                        binding,
                        expected: key.target(),
                        actual: declaration,
                    });
                }
                ExportBindingSourceV1::DeclaredCurrent { declaration }
            }
            DecodedExportBindingSourceV1::Reexport { routes } => ExportBindingSourceV1::Reexport {
                routes: routes
                    .resolve(resolver)
                    .map_err(PublicExportBindingResolutionError::Routes)?,
            },
        };
        Ok(PublicExportBindingRecordV1 { binding, source })
    }
}

impl WireEncode for DecodedPublicExportBindingRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.source.encode(encoder)
    }
}

impl WireDecode for DecodedPublicExportBindingRecordV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            binding: decoder.field(1, DecodedPersistentId::decode)?,
            source: decoder.field(2, DecodedExportBindingSourceV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalPublicExportBindingsV1 {
    records: Vec<DecodedPublicExportBindingRecordV1>,
}

impl DecodedCanonicalPublicExportBindingsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalPublicExportBindingsV1, PublicExportBindingSetValidationError<E>>
    where
        R: PublicExportBindingResolver<E>,
    {
        let mut records = Vec::<PublicExportBindingRecordV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record
                .resolve(resolver)
                .map_err(|error| PublicExportBindingSetValidationError::Record { index, error })?;
            if let Some(previous) = records.last() {
                match previous.binding.cmp(&record.binding) {
                    std::cmp::Ordering::Equal => {
                        return Err(PublicExportBindingSetValidationError::DuplicateBinding {
                            index,
                            binding: record.binding,
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(PublicExportBindingSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalPublicExportBindingsV1 { records })
    }
}

impl WireEncode for DecodedCanonicalPublicExportBindingsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalPublicExportBindingsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedPublicExportBindingRecordV1::decode(decoder))
            .map(|records| Self { records })
    }
}

pub trait PublicExportBindingResolver<E>:
    BindingResolver<E>
    + PersistentIdResolver<PersistentExportBindingId, Error = E>
    + PersistentKeyResolver<PersistentExportBindingId, ExportBindingKey, Error = E>
{
}

impl<R, E> PublicExportBindingResolver<E> for R where
    R: BindingResolver<E>
        + PersistentIdResolver<PersistentExportBindingId, Error = E>
        + PersistentKeyResolver<PersistentExportBindingId, ExportBindingKey, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicExportBindingBuildError {
    DuplicateBinding(PersistentExportBindingId),
}

impl fmt::Display for PublicExportBindingBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateBinding(binding) => {
                write!(formatter, "duplicate public export binding {binding}")
            }
        }
    }
}

impl std::error::Error for PublicExportBindingBuildError {}

#[derive(Debug)]
pub enum PublicExportBindingResolutionError<E> {
    Binding(E),
    BindingKey(E),
    Declaration(BindingIdentityResolutionError<E>),
    DeclarationMismatch {
        binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: BindableEntity,
    },
    Routes(crate::ReexportRouteSetValidationError<E>),
}

impl<E: fmt::Display> fmt::Display for PublicExportBindingResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(error) => write!(formatter, "invalid public binding identity: {error}"),
            Self::BindingKey(error) => {
                write!(
                    formatter,
                    "public binding canonical key is unavailable: {error}"
                )
            }
            Self::Declaration(error) => {
                write!(formatter, "invalid declared-current target: {error}")
            }
            Self::DeclarationMismatch {
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "public binding {binding} targets {expected:?}, but its declared-current source names {actual:?}"
            ),
            Self::Routes(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PublicExportBindingResolutionError<E> {}

#[derive(Debug)]
pub enum PublicExportBindingSetValidationError<E> {
    Record {
        index: usize,
        error: PublicExportBindingResolutionError<E>,
    },
    DuplicateBinding {
        index: usize,
        binding: PersistentExportBindingId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for PublicExportBindingSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(formatter, "invalid public export binding {index}: {error}")
            }
            Self::DuplicateBinding { index, binding } => {
                write!(
                    formatter,
                    "duplicate public export binding {binding} at index {index}"
                )
            }
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical public export binding order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for PublicExportBindingSetValidationError<E>
{
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use tests::direct_fixture;
