use super::{ProtectedSourceBuildError, wire};
use crate::{
    CallableDeclarationIdResolver, DecodedExportDefaultTemplateKeyV1, ExportDefaultTemplateKeyV1,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

/// A source-relative key in the protected default table, never a public-table
/// key or a new persistent entity identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedDefaultTemplateKeyV1(ExportDefaultTemplateKeyV1);
impl ProtectedDefaultTemplateKeyV1 {
    pub fn try_new(
        owner: CallableTemplateOrigin,
        parameter_position: u32,
    ) -> Result<Self, ProtectedSourceBuildError> {
        if matches!(owner, CallableTemplateOrigin::Accessor(_)) {
            return Err(ProtectedSourceBuildError::AccessorOwner);
        }
        Ok(Self(ExportDefaultTemplateKeyV1::new(
            owner,
            parameter_position,
        )))
    }
    pub const fn owner(self) -> CallableTemplateOrigin {
        self.0.owner()
    }
    pub const fn parameter_position(self) -> u32 {
        self.0.parameter_position()
    }
}
impl WireEncode for ProtectedDefaultTemplateKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedProtectedDefaultTemplateKeyV1(DecodedExportDefaultTemplateKeyV1);
impl DecodedProtectedDefaultTemplateKeyV1 {
    pub fn resolve<R: CallableDeclarationIdResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedDefaultTemplateKeyV1, super::ProtectedSourceResolutionError<E>> {
        let key = self
            .0
            .resolve(resolver)
            .map_err(super::ProtectedSourceResolutionError::Foundation)?;
        ProtectedDefaultTemplateKeyV1::try_new(key.owner(), key.parameter_position())
            .map_err(super::ProtectedSourceResolutionError::Build)
    }
}
impl WireDecode for DecodedProtectedDefaultTemplateKeyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        DecodedExportDefaultTemplateKeyV1::decode(decoder).map(Self)
    }
}
impl WireEncode for DecodedProtectedDefaultTemplateKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedDefaultTemplateIndexV1(u32);
impl ProtectedDefaultTemplateIndexV1 {
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl WireDecode for ProtectedDefaultTemplateIndexV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.u32().map(Self)
    }
}
impl WireEncode for ProtectedDefaultTemplateIndexV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.0))
    }
}

/// A canonical indexing projection. Only the complete default-table validator
/// establishes that every indexed key has a valid body and access coverage.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProtectedDefaultKeyIndexV1 {
    keys: Vec<ProtectedDefaultTemplateKeyV1>,
}
impl ProtectedDefaultKeyIndexV1 {
    pub fn try_new(
        mut keys: Vec<ProtectedDefaultTemplateKeyV1>,
    ) -> Result<Self, ProtectedSourceBuildError> {
        u32::try_from(keys.len()).map_err(|_| ProtectedSourceBuildError::TooMany)?;
        keys.sort_unstable();
        if keys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ProtectedSourceBuildError::DuplicateDefault);
        }
        Ok(Self { keys })
    }
    pub fn keys(&self) -> &[ProtectedDefaultTemplateKeyV1] {
        &self.keys
    }
    pub fn key(
        &self,
        index: ProtectedDefaultTemplateIndexV1,
    ) -> Result<ProtectedDefaultTemplateKeyV1, ProtectedSourceBuildError> {
        self.keys
            .get(index.0 as usize)
            .copied()
            .ok_or(ProtectedSourceBuildError::DefaultIndex)
    }
    pub fn index(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
    ) -> Result<ProtectedDefaultTemplateIndexV1, ProtectedSourceBuildError> {
        self.keys
            .binary_search(&key)
            .map(|index| ProtectedDefaultTemplateIndexV1(index as u32))
            .map_err(|_| ProtectedSourceBuildError::DefaultIndex)
    }
}
impl WireEncode for ProtectedDefaultKeyIndexV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.keys)
    }
}
