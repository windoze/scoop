use scoop_wire::{BudgetMeter, WirePath};

use super::{
    CanonicalSourceParameterShapesV1, DecodedCanonicalSourceParameterShapesV1,
    DecodedSourceParameterShapeV1, SourceParameterListValidationError,
    SourceParameterShapeResolutionError, SourceParameterShapeV1,
};
use crate::cross_cone_interface::metered_resolution::charge_name;
use crate::{MeteredInterfaceResolutionError as Error, SignatureTypeReferenceResolver};

impl DecodedSourceParameterShapeV1 {
    pub fn resolve_metered<R: SignatureTypeReferenceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<SourceParameterShapeV1, Error<SourceParameterShapeResolutionError<E>>> {
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .check_semantic_leaf(self.name.byte_len() as u64, &WirePath::root())
            .map_err(Error::Resource)?;
        charge_name(&self.name, 2, meter)?;
        self.value_type
            .charge_resolution(meter)
            .map_err(Error::Resource)?;
        self.resolve(resolver).map_err(Error::Value)
    }
}

impl DecodedCanonicalSourceParameterShapesV1 {
    pub fn resolve_metered<R: SignatureTypeReferenceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalSourceParameterShapesV1, Error<SourceParameterListValidationError<E>>>
    {
        let path = WirePath::root();
        meter
            .charge_nodes(self.parameters.len() as u64, &path)
            .map_err(Error::Resource)?;
        meter
            .charge_collection_slots((self.parameters.len() as u64).saturating_mul(2), &path)
            .map_err(Error::Resource)?;
        for parameter in &self.parameters {
            charge_name(&parameter.name, 2, meter)?;
            parameter
                .value_type
                .charge_resolution(meter)
                .map_err(Error::Resource)?;
        }
        self.resolve(resolver).map_err(Error::Value)
    }
}
