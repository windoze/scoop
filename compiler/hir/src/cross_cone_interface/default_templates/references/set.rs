use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedExportDefaultCallableReferenceV1, DecodedExportDefaultConstructorReferenceV1,
    DecodedExportDefaultFieldReferenceV1, DecodedExportDefaultGlobalReferenceV1,
    DecodedExportDefaultReferenceV1, DecodedExportDefaultSingletonReferenceV1,
    DecodedExportDefaultTypeReferenceV1, ExportDefaultCallableReferenceV1,
    ExportDefaultCallableTargetBuildError, ExportDefaultConstructorReferenceV1,
    ExportDefaultFieldReferenceV1, ExportDefaultGlobalReferenceV1,
    ExportDefaultReferenceResolutionError, ExportDefaultReferenceTargetResolutionError,
    ExportDefaultReferenceV1, ExportDefaultSingletonReferenceV1, ExportDefaultTypeReferenceV1,
};
use crate::DefaultExpressionReferenceResolver;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExportDefaultReferenceKindV1 {
    Callable,
    Constructor,
    Type,
    Global,
    Singleton,
    Field,
}

impl fmt::Display for ExportDefaultReferenceKindV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Callable => "callable",
            Self::Constructor => "constructor",
            Self::Type => "type",
            Self::Global => "global",
            Self::Singleton => "singleton",
            Self::Field => "field",
        })
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportDefaultReferenceSetV1 {
    callables: Vec<ExportDefaultCallableReferenceV1>,
    constructors: Vec<ExportDefaultConstructorReferenceV1>,
    types: Vec<ExportDefaultTypeReferenceV1>,
    globals: Vec<ExportDefaultGlobalReferenceV1>,
    singleton_values: Vec<ExportDefaultSingletonReferenceV1>,
    fields: Vec<ExportDefaultFieldReferenceV1>,
}

impl ExportDefaultReferenceSetV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        callables: Vec<ExportDefaultCallableReferenceV1>,
        constructors: Vec<ExportDefaultConstructorReferenceV1>,
        types: Vec<ExportDefaultTypeReferenceV1>,
        globals: Vec<ExportDefaultGlobalReferenceV1>,
        singleton_values: Vec<ExportDefaultSingletonReferenceV1>,
        fields: Vec<ExportDefaultFieldReferenceV1>,
    ) -> Result<Self, ExportDefaultReferenceSetBuildError> {
        for (index, reference) in callables.iter().enumerate() {
            reference.target().validate().map_err(|error| {
                ExportDefaultReferenceSetBuildError::CallableTarget { index, error }
            })?;
        }
        Ok(Self {
            callables: canonicalize(callables, ExportDefaultReferenceKindV1::Callable)?,
            constructors: canonicalize(constructors, ExportDefaultReferenceKindV1::Constructor)?,
            types: canonicalize(types, ExportDefaultReferenceKindV1::Type)?,
            globals: canonicalize(globals, ExportDefaultReferenceKindV1::Global)?,
            singleton_values: canonicalize(
                singleton_values,
                ExportDefaultReferenceKindV1::Singleton,
            )?,
            fields: canonicalize(fields, ExportDefaultReferenceKindV1::Field)?,
        })
    }

    pub fn callables(&self) -> &[ExportDefaultCallableReferenceV1] {
        &self.callables
    }

    pub fn constructors(&self) -> &[ExportDefaultConstructorReferenceV1] {
        &self.constructors
    }

    pub fn types(&self) -> &[ExportDefaultTypeReferenceV1] {
        &self.types
    }

    pub fn globals(&self) -> &[ExportDefaultGlobalReferenceV1] {
        &self.globals
    }

    pub fn singleton_values(&self) -> &[ExportDefaultSingletonReferenceV1] {
        &self.singleton_values
    }

    pub fn fields(&self) -> &[ExportDefaultFieldReferenceV1] {
        &self.fields
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
            && self.constructors.is_empty()
            && self.types.is_empty()
            && self.globals.is_empty()
            && self.singleton_values.is_empty()
            && self.fields.is_empty()
    }
}

impl WireEncode for ExportDefaultReferenceSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encode_field(encoder, 1, &self.callables)?;
        encode_field(encoder, 2, &self.constructors)?;
        encode_field(encoder, 3, &self.types)?;
        encode_field(encoder, 4, &self.globals)?;
        encode_field(encoder, 5, &self.singleton_values)?;
        encode_field(encoder, 6, &self.fields)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportDefaultReferenceSetV1 {
    callables: Vec<DecodedExportDefaultCallableReferenceV1>,
    constructors: Vec<DecodedExportDefaultConstructorReferenceV1>,
    types: Vec<DecodedExportDefaultTypeReferenceV1>,
    globals: Vec<DecodedExportDefaultGlobalReferenceV1>,
    singleton_values: Vec<DecodedExportDefaultSingletonReferenceV1>,
    fields: Vec<DecodedExportDefaultFieldReferenceV1>,
}

impl DecodedExportDefaultReferenceSetV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportDefaultReferenceSetV1, ExportDefaultReferenceSetValidationError<E>>
    where
        R: DefaultExpressionReferenceResolver<E>,
    {
        self.resolve_metered(
            resolver,
            &mut scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default()),
            &scoop_wire::WirePath::root(),
        )
    }

    pub fn resolve_metered<R, E>(
        self,
        resolver: &mut R,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<ExportDefaultReferenceSetV1, ExportDefaultReferenceSetValidationError<E>>
    where
        R: DefaultExpressionReferenceResolver<E>,
    {
        let callables = resolve_set(
            self.callables,
            resolver,
            meter,
            path,
            ExportDefaultReferenceKindV1::Callable,
            |target, resolver| {
                target
                    .resolve(resolver)
                    .map_err(ExportDefaultReferenceTargetResolutionError::Callable)
            },
        )?;
        let constructors = resolve_set(
            self.constructors,
            resolver,
            meter,
            path,
            ExportDefaultReferenceKindV1::Constructor,
            |target, resolver| {
                target
                    .resolve(resolver)
                    .map_err(ExportDefaultReferenceTargetResolutionError::Constructor)
            },
        )?;
        let types = resolve_set(
            self.types,
            resolver,
            meter,
            path,
            ExportDefaultReferenceKindV1::Type,
            |target, resolver| {
                target
                    .resolve(resolver)
                    .map_err(ExportDefaultReferenceTargetResolutionError::Type)
            },
        )?;
        let globals = resolve_set(
            self.globals,
            resolver,
            meter,
            path,
            ExportDefaultReferenceKindV1::Global,
            |target, resolver| {
                resolver
                    .resolve(target)
                    .map_err(ExportDefaultReferenceTargetResolutionError::Global)
            },
        )?;
        let singleton_values = resolve_set(
            self.singleton_values,
            resolver,
            meter,
            path,
            ExportDefaultReferenceKindV1::Singleton,
            |target, resolver| {
                resolver
                    .resolve(target)
                    .map_err(ExportDefaultReferenceTargetResolutionError::Singleton)
            },
        )?;
        let fields = resolve_set(
            self.fields,
            resolver,
            meter,
            path,
            ExportDefaultReferenceKindV1::Field,
            |target, resolver| {
                target
                    .resolve(resolver)
                    .map_err(ExportDefaultReferenceTargetResolutionError::Field)
            },
        )?;
        Ok(ExportDefaultReferenceSetV1 {
            callables,
            constructors,
            types,
            globals,
            singleton_values,
            fields,
        })
    }
}

impl WireEncode for DecodedExportDefaultReferenceSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encode_field(encoder, 1, &self.callables)?;
        encode_field(encoder, 2, &self.constructors)?;
        encode_field(encoder, 3, &self.types)?;
        encode_field(encoder, 4, &self.globals)?;
        encode_field(encoder, 5, &self.singleton_values)?;
        encode_field(encoder, 6, &self.fields)
    }
}

impl WireDecode for DecodedExportDefaultReferenceSetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            callables: decode_field(decoder, 1)?,
            constructors: decode_field(decoder, 2)?,
            types: decode_field(decoder, 3)?,
            globals: decode_field(decoder, 4)?,
            singleton_values: decode_field(decoder, 5)?,
            fields: decode_field(decoder, 6)?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceSetBuildError {
    TooMany(ExportDefaultReferenceKindV1),
    Duplicate(ExportDefaultReferenceKindV1),
    CallableTarget {
        index: usize,
        error: ExportDefaultCallableTargetBuildError,
    },
}

impl fmt::Display for ExportDefaultReferenceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany(kind) => write!(formatter, "default {kind} reference count exceeds u32"),
            Self::Duplicate(kind) => write!(formatter, "duplicate default {kind} reference"),
            Self::CallableTarget { index, error } => {
                write!(
                    formatter,
                    "invalid default callable reference {index}: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ExportDefaultReferenceSetBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceSetValidationError<E> {
    Resource(WireError),
    TooMany(ExportDefaultReferenceKindV1),
    Record {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        error: ExportDefaultReferenceResolutionError<E>,
    },
    Duplicate {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
    },
    NonCanonicalOrder {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for ExportDefaultReferenceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::TooMany(kind) => write!(formatter, "default {kind} reference count exceeds u32"),
            Self::Record { kind, index, error } => {
                write!(
                    formatter,
                    "invalid default {kind} reference {index}: {error}"
                )
            }
            Self::Duplicate { kind, index } => {
                write!(
                    formatter,
                    "duplicate default {kind} reference at index {index}"
                )
            }
            Self::NonCanonicalOrder { kind, index } => write!(
                formatter,
                "non-canonical default {kind} reference order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultReferenceSetValidationError<E>
{
}

fn canonicalize<T: Ord>(
    mut records: Vec<T>,
    kind: ExportDefaultReferenceKindV1,
) -> Result<Vec<T>, ExportDefaultReferenceSetBuildError> {
    u32::try_from(records.len()).map_err(|_| ExportDefaultReferenceSetBuildError::TooMany(kind))?;
    records.sort_unstable();
    if records.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ExportDefaultReferenceSetBuildError::Duplicate(kind));
    }
    Ok(records)
}

fn resolve_set<R, E, D, T>(
    records: Vec<DecodedExportDefaultReferenceV1<D>>,
    resolver: &mut R,
    meter: &mut scoop_wire::BudgetMeter,
    path: &scoop_wire::WirePath,
    kind: ExportDefaultReferenceKindV1,
    mut resolve_target: impl FnMut(
        D,
        &mut R,
    ) -> Result<T, ExportDefaultReferenceTargetResolutionError<E>>,
) -> Result<Vec<ExportDefaultReferenceV1<T>>, ExportDefaultReferenceSetValidationError<E>>
where
    R: DefaultExpressionReferenceResolver<E>,
    T: Ord,
{
    u32::try_from(records.len())
        .map_err(|_| ExportDefaultReferenceSetValidationError::TooMany(kind))?;
    meter
        .check_table_entries(records.len() as u64, path)
        .map_err(ExportDefaultReferenceSetValidationError::Resource)?;
    let mut resolved: Vec<ExportDefaultReferenceV1<T>> = Vec::new();
    meter
        .try_reserve_collection_slots(&mut resolved, records.len(), path)
        .map_err(ExportDefaultReferenceSetValidationError::Resource)?;
    for (index, record) in records.into_iter().enumerate() {
        let record = record
            .resolve_with(
                resolver,
                meter,
                &path.clone().field(kind as u32 + 1).index(index as u64),
                |target, resolver| resolve_target(target, resolver),
            )
            .map_err(|error| ExportDefaultReferenceSetValidationError::Record {
                kind,
                index,
                error,
            })?;
        if let Some(previous) = resolved.last() {
            previous
                .witness()
                .charge_comparison(record.witness(), meter, path)
                .map_err(ExportDefaultReferenceSetValidationError::Resource)?;
            match previous.cmp(&record) {
                std::cmp::Ordering::Equal => {
                    return Err(ExportDefaultReferenceSetValidationError::Duplicate {
                        kind,
                        index,
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(
                        ExportDefaultReferenceSetValidationError::NonCanonicalOrder { kind, index },
                    );
                }
                std::cmp::Ordering::Less => {}
            }
        }
        resolved.push(record);
    }
    Ok(resolved)
}

fn encode_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    records: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(records.len() as u64)?;
    for record in records {
        record.encode(encoder)?;
    }
    Ok(())
}

fn decode_field<T: WireDecode>(
    decoder: &mut Decoder<'_, '_>,
    field: u32,
) -> Result<Vec<T>, WireError> {
    decoder.field(field, |decoder| {
        decoder.decode_array(|decoder, _| T::decode(decoder))
    })
}
