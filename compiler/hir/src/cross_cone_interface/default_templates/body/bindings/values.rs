use std::fmt;

use scoop_identity::{DecodedSignatureTypeKey, LocalValueSelector, SignatureTypeKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{
    CanonicalBooleanV1, SignatureTypeReferenceResolver, TemplateLocalIndexResolver,
    TemplateLocalSelectorResolver,
};

/// One typed immutable local used while a portable binding plan executes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBindingTemporaryV1 {
    local: LocalValueSelector,
    value_type: SignatureTypeKey,
}

impl DefaultBindingTemporaryV1 {
    pub const fn new(local: LocalValueSelector, value_type: SignatureTypeKey) -> Self {
        Self { local, value_type }
    }

    pub const fn local(&self) -> &LocalValueSelector {
        &self.local
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub fn index_local<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultBindingTemporaryV1<'_>, DefaultBindingTemporaryIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let local_index = resolver
            .resolve_template_local_index(&self.local)
            .map_err(DefaultBindingTemporaryIndexError::Local)?;
        Ok(IndexedDefaultBindingTemporaryV1 {
            temporary: self,
            local_index,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultBindingTemporaryV1 {
    local_index: u32,
    value_type: DecodedSignatureTypeKey,
}

impl DecodedDefaultBindingTemporaryV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultBindingTemporaryV1, DefaultBindingTemporaryResolutionError<E, L::Error>>
    where
        R: SignatureTypeReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        Ok(DefaultBindingTemporaryV1 {
            local: locals
                .resolve_template_local_selector(self.local_index)
                .map_err(DefaultBindingTemporaryResolutionError::Local)?,
            value_type: self
                .value_type
                .resolve(resolver)
                .map_err(DefaultBindingTemporaryResolutionError::ValueType)?,
        })
    }
}

impl WireEncode for DecodedDefaultBindingTemporaryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_typed_local(encoder, self.local_index, &self.value_type)
    }
}

impl WireDecode for DecodedDefaultBindingTemporaryV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            local_index: decoder.field(1, Decoder::u32)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct IndexedDefaultBindingTemporaryV1<'a> {
    temporary: &'a DefaultBindingTemporaryV1,
    local_index: u32,
}

impl WireEncode for IndexedDefaultBindingTemporaryV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_typed_local(encoder, self.local_index, &self.temporary.value_type)
    }
}

/// One typed user-visible local introduced by a portable binding plan.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBindingLeafV1 {
    local: LocalValueSelector,
    value_type: SignatureTypeKey,
    mutable: CanonicalBooleanV1,
}

impl DefaultBindingLeafV1 {
    pub const fn new(
        local: LocalValueSelector,
        value_type: SignatureTypeKey,
        mutable: CanonicalBooleanV1,
    ) -> Self {
        Self {
            local,
            value_type,
            mutable,
        }
    }

    pub const fn local(&self) -> &LocalValueSelector {
        &self.local
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn mutable(&self) -> CanonicalBooleanV1 {
        self.mutable
    }

    pub fn index_local<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultBindingLeafV1<'_>, DefaultBindingLeafIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let local_index = resolver
            .resolve_template_local_index(&self.local)
            .map_err(DefaultBindingLeafIndexError::Local)?;
        Ok(IndexedDefaultBindingLeafV1 {
            leaf: self,
            local_index,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultBindingLeafV1 {
    local_index: u32,
    value_type: DecodedSignatureTypeKey,
    mutable: CanonicalBooleanV1,
}

impl DecodedDefaultBindingLeafV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultBindingLeafV1, DefaultBindingLeafResolutionError<E, L::Error>>
    where
        R: SignatureTypeReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        Ok(DefaultBindingLeafV1 {
            local: locals
                .resolve_template_local_selector(self.local_index)
                .map_err(DefaultBindingLeafResolutionError::Local)?,
            value_type: self
                .value_type
                .resolve(resolver)
                .map_err(DefaultBindingLeafResolutionError::ValueType)?,
            mutable: self.mutable,
        })
    }
}

impl WireEncode for DecodedDefaultBindingLeafV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_leaf(encoder, self.local_index, &self.value_type, self.mutable)
    }
}

impl WireDecode for DecodedDefaultBindingLeafV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            local_index: decoder.field(1, Decoder::u32)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            mutable: decoder.field(3, CanonicalBooleanV1::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct IndexedDefaultBindingLeafV1<'a> {
    leaf: &'a DefaultBindingLeafV1,
    local_index: u32,
}

impl WireEncode for IndexedDefaultBindingLeafV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_leaf(
            encoder,
            self.local_index,
            &self.leaf.value_type,
            self.leaf.mutable,
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBindingTemporaryResolutionError<E, L> {
    Local(L),
    ValueType(E),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display
    for DefaultBindingTemporaryResolutionError<E, L>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "invalid default binding temporary: {error}"),
            Self::ValueType(error) => {
                write!(formatter, "invalid default binding temporary type: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultBindingTemporaryResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBindingTemporaryIndexError<E> {
    Local(E),
}

impl<E: fmt::Display> fmt::Display for DefaultBindingTemporaryIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => {
                write!(formatter, "cannot index default binding temporary: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultBindingTemporaryIndexError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBindingLeafResolutionError<E, L> {
    Local(L),
    ValueType(E),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultBindingLeafResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "invalid default binding leaf: {error}"),
            Self::ValueType(error) => {
                write!(formatter, "invalid default binding leaf type: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultBindingLeafResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBindingLeafIndexError<E> {
    Local(E),
}

impl<E: fmt::Display> fmt::Display for DefaultBindingLeafIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "cannot index default binding leaf: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultBindingLeafIndexError<E> {}

fn encode_typed_local(
    encoder: &mut Encoder,
    local_index: u32,
    value_type: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(local_index))?;
    encoder.field(2)?;
    value_type.encode(encoder)
}

fn encode_leaf(
    encoder: &mut Encoder,
    local_index: u32,
    value_type: &impl WireEncode,
    mutable: CanonicalBooleanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(local_index))?;
    encoder.field(2)?;
    value_type.encode(encoder)?;
    encoder.field(3)?;
    mutable.encode(encoder)
}

#[cfg(test)]
mod tests;
