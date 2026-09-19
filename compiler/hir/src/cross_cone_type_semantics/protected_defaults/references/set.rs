use std::{cmp::Ordering, fmt};

use scoop_wire::{Encoder, WireEncode};

use super::*;
use crate::cross_cone_type_semantics::wire;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProtectedDefaultReferenceKindV1 {
    Callable,
    Constructor,
    Type,
    Global,
    Singleton,
    Field,
}
impl fmt::Display for ProtectedDefaultReferenceKindV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Callable => "callable",
            Self::Constructor => "constructor",
            Self::Type => "type",
            Self::Global => "global",
            Self::Singleton => "singleton",
            Self::Field => "field",
        })
    }
}

/// Canonical transported references; body closure and access require independent replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedDefaultReferenceSetV1 {
    pub(super) callables: Vec<ProtectedDefaultCallableReferenceV1>,
    pub(super) constructors: Vec<ProtectedDefaultConstructorReferenceV1>,
    pub(super) types: Vec<ProtectedDefaultTypeReferenceV1>,
    pub(super) globals: Vec<ProtectedDefaultGlobalReferenceV1>,
    pub(super) singleton_values: Vec<ProtectedDefaultSingletonReferenceV1>,
    pub(super) fields: Vec<ProtectedDefaultFieldReferenceV1>,
}
impl ProtectedDefaultReferenceSetV1 {
    pub fn try_new(
        callables: Vec<ProtectedDefaultCallableReferenceV1>,
        constructors: Vec<ProtectedDefaultConstructorReferenceV1>,
        types: Vec<ProtectedDefaultTypeReferenceV1>,
        globals: Vec<ProtectedDefaultGlobalReferenceV1>,
        singleton_values: Vec<ProtectedDefaultSingletonReferenceV1>,
        fields: Vec<ProtectedDefaultFieldReferenceV1>,
    ) -> Result<Self, ProtectedDefaultReferenceSetBuildError> {
        for (index, record) in callables.iter().enumerate() {
            record.target().validate().map_err(|error| {
                ProtectedDefaultReferenceSetBuildError::CallableTarget { index, error }
            })?;
        }
        Ok(Self {
            callables: canonicalize(callables, ProtectedDefaultReferenceKindV1::Callable)?,
            constructors: canonicalize(constructors, ProtectedDefaultReferenceKindV1::Constructor)?,
            types: canonicalize(types, ProtectedDefaultReferenceKindV1::Type)?,
            globals: canonicalize(globals, ProtectedDefaultReferenceKindV1::Global)?,
            singleton_values: canonicalize(
                singleton_values,
                ProtectedDefaultReferenceKindV1::Singleton,
            )?,
            fields: canonicalize(fields, ProtectedDefaultReferenceKindV1::Field)?,
        })
    }
    pub fn callables(&self) -> &[ProtectedDefaultCallableReferenceV1] {
        &self.callables
    }
    pub fn constructors(&self) -> &[ProtectedDefaultConstructorReferenceV1] {
        &self.constructors
    }
    pub fn types(&self) -> &[ProtectedDefaultTypeReferenceV1] {
        &self.types
    }
    pub fn globals(&self) -> &[ProtectedDefaultGlobalReferenceV1] {
        &self.globals
    }
    pub fn singleton_values(&self) -> &[ProtectedDefaultSingletonReferenceV1] {
        &self.singleton_values
    }
    pub fn fields(&self) -> &[ProtectedDefaultFieldReferenceV1] {
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
impl WireEncode for ProtectedDefaultReferenceSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        wire::sequence(encoder, &self.callables)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.constructors)?;
        encoder.field(3)?;
        wire::sequence(encoder, &self.types)?;
        encoder.field(4)?;
        wire::sequence(encoder, &self.globals)?;
        encoder.field(5)?;
        wire::sequence(encoder, &self.singleton_values)?;
        encoder.field(6)?;
        wire::sequence(encoder, &self.fields)
    }
}
fn canonicalize<T: Ord>(
    mut records: Vec<ProtectedDefaultReferenceV1<T>>,
    kind: ProtectedDefaultReferenceKindV1,
) -> Result<Vec<ProtectedDefaultReferenceV1<T>>, ProtectedDefaultReferenceSetBuildError> {
    u32::try_from(records.len())
        .map_err(|_| ProtectedDefaultReferenceSetBuildError::TooMany(kind))?;
    records.sort_unstable_by(compare_key);
    for (offset, pair) in records.windows(2).enumerate() {
        if compare_key(&pair[0], &pair[1]) == Ordering::Equal {
            return Err(ProtectedDefaultReferenceSetBuildError::Duplicate {
                kind,
                index: offset + 1,
            });
        }
    }
    Ok(records)
}
fn compare_key<T: Ord>(
    left: &ProtectedDefaultReferenceV1<T>,
    right: &ProtectedDefaultReferenceV1<T>,
) -> Ordering {
    left.target()
        .cmp(right.target())
        .then_with(|| left.definition_origin().cmp(right.definition_origin()))
}
