use scoop_identity::{
    ConeIdentity, DecodedCallableMaterialization, DecodedConcreteExpressionOrigin,
    DecodedPersistentId, PersistentCallableApplicationId, PersistentConstructorId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentKeyResolver, PersistentPropertyAccessorId,
    PersistentSourceContextId, SourceContextKey,
};
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::*;

pub trait HirDependencyCallSiteResolver<E>:
    PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl<R, E> HirDependencyCallSiteResolver<E> for R where
    R: PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedHirDependencyCallSiteV1 {
    root: DecodedCallableMaterialization,
    expression_index: u32,
    origin: DecodedConcreteExpressionOrigin,
    arguments: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    result: DecodedPersistentId<PersistentExactTypeId>,
    reason: DecodedHirDependencyCallReasonV1,
    receiver: crate::SourceCallReceiver<DecodedPersistentId<PersistentExactTypeId>>,
}

impl DecodedHirDependencyCallSiteV1 {
    pub fn resolve<R: HirDependencyCallSiteResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<HirDependencyCallSiteV1, HirDependencyCallSiteResolutionError<E>> {
        use HirDependencyCallSiteResolutionError as Error;
        let bytes = scoop_wire::encoded_length(&self).map_err(|_| {
            Error::Resource(WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })?;
        meter
            .charge_owned_bytes(bytes, path)
            .map_err(Error::Resource)?;
        meter.charge_work(bytes, path).map_err(Error::Resource)?;
        meter
            .charge_nodes(
                2 + self.arguments.len() as u64 + u64::from(self.receiver.has_receiver()),
                path,
            )
            .map_err(Error::Resource)?;
        let position = ExecutableExpressionPosition {
            root: self.root.resolve(resolver).map_err(Error::Identity)?,
            expression_index: self.expression_index,
        };
        let origin = self.origin.resolve(resolver).map_err(Error::Origin)?;
        let mut arguments = Vec::new();
        meter
            .try_reserve_collection_slots(&mut arguments, self.arguments.len(), path)
            .map_err(Error::Resource)?;
        for argument in self.arguments {
            arguments.push(resolver.resolve(argument).map_err(Error::Identity)?);
        }
        let result = resolver.resolve(self.result).map_err(Error::Identity)?;
        let reason = self.reason.resolve(resolver).map_err(Error::Identity)?;
        let receiver = self
            .receiver
            .try_map(|ty| resolver.resolve(ty))
            .map_err(Error::Identity)?;
        HirDependencyCallSiteV1::try_new_with_reason(
            position, origin, arguments, result, reason, receiver,
        )
        .map_err(Error::Shape)
    }
}

impl WireEncode for DecodedHirDependencyCallSiteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.root.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.expression_index))?;
        encoder.field(3)?;
        self.origin.encode(encoder)?;
        encoder.field(4)?;
        encoder.array(self.arguments.len() as u64)?;
        for argument in &self.arguments {
            argument.encode(encoder)?;
        }
        encoder.field(5)?;
        self.result.encode(encoder)?;
        encoder.field(6)?;
        self.reason.encode(encoder)?;
        encoder.field(7)?;
        self.receiver.encode(encoder)
    }
}

impl WireDecode for DecodedHirDependencyCallSiteV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            root: decoder.field(1, DecodedCallableMaterialization::decode)?,
            expression_index: decoder.field(2, Decoder::u32)?,
            origin: decoder.field(3, DecodedConcreteExpressionOrigin::decode)?,
            arguments: decoder
                .field(4, |d| d.decode_array(|d, _| DecodedPersistentId::decode(d)))?,
            result: decoder.field(5, DecodedPersistentId::decode)?,
            reason: decoder.field(6, DecodedHirDependencyCallReasonV1::decode)?,
            receiver: decoder.field(7, crate::SourceCallReceiver::decode)?,
        })
    }
}
