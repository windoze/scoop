use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath, encoded_length,
};

use super::super::{
    DecodedCanonicalProtectedDefaultExpressionUsesV1, DecodedProtectedDefaultAccessWitnessV1,
    ProtectedDefaultAccessWitnessResolver,
};
use super::{ProtectedDefaultReferenceResolutionError, ProtectedDefaultReferenceV1};
use crate::{DecodedExportDefinitionSourceV1, DefaultExpressionReferenceResolver};

pub trait ProtectedDefaultReferenceResolver<E>:
    DefaultExpressionReferenceResolver<E> + ProtectedDefaultAccessWitnessResolver<E>
{
}
impl<R, E> ProtectedDefaultReferenceResolver<E> for R where
    R: DefaultExpressionReferenceResolver<E> + ProtectedDefaultAccessWitnessResolver<E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedDefaultReferenceV1<T> {
    target: T,
    definition_origin: DecodedExportDefinitionSourceV1,
    witness: DecodedProtectedDefaultAccessWitnessV1,
    uses: DecodedCanonicalProtectedDefaultExpressionUsesV1,
}
impl<T> DecodedProtectedDefaultReferenceV1<T> {
    pub(crate) fn resolve_with<R: ProtectedDefaultReferenceResolver<E>, E, U>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        resolve_target: impl FnOnce(
            T,
            &mut R,
            &mut BudgetMeter,
        ) -> Result<U, ProtectedDefaultReferenceResolutionError<E>>,
    ) -> Result<ProtectedDefaultReferenceV1<U>, ProtectedDefaultReferenceResolutionError<E>> {
        use ProtectedDefaultReferenceResolutionError as Error;
        let path = WirePath::root();
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let target = resolve_target(self.target, resolver, meter)?;
        let bytes = encoded_length(&self.definition_origin).map_err(Error::Encoding)?;
        meter
            .charge_owned_bytes(bytes, &path)
            .map_err(Error::Resource)?;
        meter.charge_work(bytes, &path).map_err(Error::Resource)?;
        let origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(Error::DefinitionOrigin)?;
        let witness = self
            .witness
            .resolve(resolver, meter)
            .map_err(Error::Witness)?;
        let uses = self.uses.resolve(meter).map_err(Error::Uses)?;
        Ok(ProtectedDefaultReferenceV1::new(
            target, origin, witness, uses,
        ))
    }
}
impl<T: WireEncode> WireEncode for DecodedProtectedDefaultReferenceV1<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.definition_origin.encode(encoder)?;
        encoder.field(3)?;
        self.witness.encode(encoder)?;
        encoder.field(4)?;
        self.uses.encode(encoder)
    }
}
impl<T: WireDecode> WireDecode for DecodedProtectedDefaultReferenceV1<T> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            target: decoder.field(1, T::decode)?,
            definition_origin: decoder.field(2, DecodedExportDefinitionSourceV1::decode)?,
            witness: decoder.field(3, DecodedProtectedDefaultAccessWitnessV1::decode)?,
            uses: decoder.field(4, DecodedCanonicalProtectedDefaultExpressionUsesV1::decode)?,
        })
    }
}
