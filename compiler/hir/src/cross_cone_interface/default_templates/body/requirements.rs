//! A requirement belongs to an original callable's canonical binder contract.

use crate::{
    DecodedDefaultCallableDeclarationV1, DefaultCallableDeclarationV1,
    DefaultCallableReferenceResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultMemberRequirementRefV1 {
    pub declaration: DefaultCallableDeclarationV1,
    pub parameter: u32,
    pub requirement: u32,
}

impl WireEncode for DefaultMemberRequirementRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, &self.declaration, self.parameter, self.requirement)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedDefaultMemberRequirementRefV1 {
    declaration: DecodedDefaultCallableDeclarationV1,
    parameter: u32,
    requirement: u32,
}

impl DecodedDefaultMemberRequirementRefV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DefaultMemberRequirementRefV1, E>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        Ok(DefaultMemberRequirementRefV1 {
            declaration: self.declaration.resolve(resolver)?,
            parameter: self.parameter,
            requirement: self.requirement,
        })
    }
}

impl WireDecode for DecodedDefaultMemberRequirementRefV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedDefaultCallableDeclarationV1::decode)?,
            parameter: decoder.field(2, Decoder::u32)?,
            requirement: decoder.field(3, Decoder::u32)?,
        })
    }
}

impl WireEncode for DecodedDefaultMemberRequirementRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, &self.declaration, self.parameter, self.requirement)
    }
}

fn encode(
    encoder: &mut Encoder,
    declaration: &impl WireEncode,
    parameter: u32,
    requirement: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    declaration.encode(encoder)?;
    encoder.field(2)?;
    encoder.unsigned(parameter.into())?;
    encoder.field(3)?;
    encoder.unsigned(requirement.into())
}

mod bindings;
pub use bindings::*;

#[cfg(test)]
mod tests;
