use scoop_identity::{
    DecodedCallableMaterialization, DecodedConcreteExpressionOrigin, DecodedPersistentId,
};
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::*;
use crate::HirDependencyCallSiteResolver;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedHirDependencyTypeSiteV1 {
    root: DecodedCallableMaterialization,
    expression_index: u32,
    origin: DecodedConcreteExpressionOrigin,
    role: HirExpressionTypeRoleV1,
    exact: DecodedPersistentId<PersistentExactTypeId>,
}

impl DecodedHirDependencyTypeSiteV1 {
    pub fn resolve<R: HirDependencyCallSiteResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<HirDependencyTypeSiteV1, HirDependencyTypeSiteResolutionError<E>> {
        use HirDependencyTypeSiteResolutionError as Error;
        let bytes = scoop_wire::encoded_length(&self).map_err(|_| {
            WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            )
        })?;
        meter.charge_owned_bytes(bytes, path)?;
        meter.charge_work(bytes, path)?;
        meter.charge_nodes(2, path)?;
        let position = ExecutableExpressionPosition {
            root: self.root.resolve(resolver).map_err(Error::Identity)?,
            expression_index: self.expression_index,
        };
        let origin = self.origin.resolve(resolver).map_err(Error::Origin)?;
        let exact = resolver.resolve(self.exact).map_err(Error::Identity)?;
        Ok(HirDependencyTypeSiteV1::new(
            position, origin, self.role, exact,
        ))
    }
}

impl WireEncode for DecodedHirDependencyTypeSiteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.root.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.expression_index))?;
        encoder.field(3)?;
        self.origin.encode(encoder)?;
        encoder.field(4)?;
        self.role.encode(encoder)?;
        encoder.field(5)?;
        self.exact.encode(encoder)
    }
}

impl WireDecode for DecodedHirDependencyTypeSiteV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            root: decoder.field(1, DecodedCallableMaterialization::decode)?,
            expression_index: decoder.field(2, Decoder::u32)?,
            origin: decoder.field(3, DecodedConcreteExpressionOrigin::decode)?,
            role: decoder.field(4, HirExpressionTypeRoleV1::decode)?,
            exact: decoder.field(5, DecodedPersistentId::decode)?,
        })
    }
}
