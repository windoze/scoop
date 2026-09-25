use super::*;
use crate::cross_cone_type_semantics::wire;
use crate::*;
use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentObjectValueId, PersistentPropertyId,
    SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

/// Six occurrence sequences in source traversal order, retaining multiplicity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultSourceReferencesV1 {
    pub(super) callables: Vec<DefaultSourceReferenceV1<ExportDefaultCallableTargetV1>>,
    pub(super) constructors: Vec<DefaultSourceReferenceV1<DefaultConstructorRefV1>>,
    pub(super) types: Vec<DefaultSourceReferenceV1<SignatureTypeKey>>,
    pub(super) globals: Vec<DefaultSourceReferenceV1<PersistentPropertyId>>,
    pub(super) singleton_values: Vec<DefaultSourceReferenceV1<PersistentObjectValueId>>,
    pub(super) fields: Vec<DefaultSourceReferenceV1<DefaultFieldRefV1>>,
}
impl DefaultSourceReferencesV1 {
    pub fn try_new(
        callables: Vec<DefaultSourceReferenceV1<ExportDefaultCallableTargetV1>>,
        constructors: Vec<DefaultSourceReferenceV1<DefaultConstructorRefV1>>,
        types: Vec<DefaultSourceReferenceV1<SignatureTypeKey>>,
        globals: Vec<DefaultSourceReferenceV1<PersistentPropertyId>>,
        singleton_values: Vec<DefaultSourceReferenceV1<PersistentObjectValueId>>,
        fields: Vec<DefaultSourceReferenceV1<DefaultFieldRefV1>>,
    ) -> Result<Self, DefaultSourceReferencesBuildError> {
        u32::try_from(callables.len()).map_err(|_| {
            DefaultSourceReferencesBuildError::TooMany(ExportDefaultReferenceKindV1::Callable)
        })?;
        u32::try_from(constructors.len()).map_err(|_| {
            DefaultSourceReferencesBuildError::TooMany(ExportDefaultReferenceKindV1::Constructor)
        })?;
        u32::try_from(types.len()).map_err(|_| {
            DefaultSourceReferencesBuildError::TooMany(ExportDefaultReferenceKindV1::Type)
        })?;
        u32::try_from(globals.len()).map_err(|_| {
            DefaultSourceReferencesBuildError::TooMany(ExportDefaultReferenceKindV1::Global)
        })?;
        u32::try_from(singleton_values.len()).map_err(|_| {
            DefaultSourceReferencesBuildError::TooMany(ExportDefaultReferenceKindV1::Singleton)
        })?;
        u32::try_from(fields.len()).map_err(|_| {
            DefaultSourceReferencesBuildError::TooMany(ExportDefaultReferenceKindV1::Field)
        })?;
        for (index, record) in callables.iter().enumerate() {
            record.target().validate().map_err(|error| {
                DefaultSourceReferencesBuildError::CallableTarget { index, error }
            })?;
        }
        Ok(Self {
            callables,
            constructors,
            types,
            globals,
            singleton_values,
            fields,
        })
    }
    pub fn callables(&self) -> &[DefaultSourceReferenceV1<ExportDefaultCallableTargetV1>] {
        &self.callables
    }
    pub fn constructors(&self) -> &[DefaultSourceReferenceV1<DefaultConstructorRefV1>] {
        &self.constructors
    }
    pub fn types(&self) -> &[DefaultSourceReferenceV1<SignatureTypeKey>] {
        &self.types
    }
    pub fn globals(&self) -> &[DefaultSourceReferenceV1<PersistentPropertyId>] {
        &self.globals
    }
    pub fn singleton_values(&self) -> &[DefaultSourceReferenceV1<PersistentObjectValueId>] {
        &self.singleton_values
    }
    pub fn fields(&self) -> &[DefaultSourceReferenceV1<DefaultFieldRefV1>] {
        &self.fields
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultSourceReferencesV1 {
    pub(super) callables:
        Vec<DecodedDefaultSourceReferenceV1<DecodedExportDefaultCallableTargetV1>>,
    pub(super) constructors: Vec<DecodedDefaultSourceReferenceV1<DecodedDefaultConstructorRefV1>>,
    pub(super) types: Vec<DecodedDefaultSourceReferenceV1<DecodedSignatureTypeKey>>,
    pub(super) globals:
        Vec<DecodedDefaultSourceReferenceV1<DecodedPersistentId<PersistentPropertyId>>>,
    pub(super) singleton_values:
        Vec<DecodedDefaultSourceReferenceV1<DecodedPersistentId<PersistentObjectValueId>>>,
    pub(super) fields: Vec<DecodedDefaultSourceReferenceV1<DecodedDefaultFieldRefV1>>,
}
impl WireDecode for DecodedDefaultSourceReferencesV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(6)?;
        Ok(Self {
            callables: d.field(1, |d| {
                d.decode_array(|d, _| DecodedDefaultSourceReferenceV1::decode(d))
            })?,
            constructors: d.field(2, |d| {
                d.decode_array(|d, _| DecodedDefaultSourceReferenceV1::decode(d))
            })?,
            types: d.field(3, |d| {
                d.decode_array(|d, _| DecodedDefaultSourceReferenceV1::decode(d))
            })?,
            globals: d.field(4, |d| {
                d.decode_array(|d, _| DecodedDefaultSourceReferenceV1::decode(d))
            })?,
            singleton_values: d.field(5, |d| {
                d.decode_array(|d, _| DecodedDefaultSourceReferenceV1::decode(d))
            })?,
            fields: d.field(6, |d| {
                d.decode_array(|d, _| DecodedDefaultSourceReferenceV1::decode(d))
            })?,
        })
    }
}
macro_rules! encode_sequences {
    ($name:ty) => {
        impl WireEncode for $name {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(6)?;
                e.field(1)?;
                wire::sequence(e, &self.callables)?;
                e.field(2)?;
                wire::sequence(e, &self.constructors)?;
                e.field(3)?;
                wire::sequence(e, &self.types)?;
                e.field(4)?;
                wire::sequence(e, &self.globals)?;
                e.field(5)?;
                wire::sequence(e, &self.singleton_values)?;
                e.field(6)?;
                wire::sequence(e, &self.fields)?;
                Ok(())
            }
        }
    };
}
encode_sequences!(DefaultSourceReferencesV1);
encode_sequences!(DecodedDefaultSourceReferencesV1);
