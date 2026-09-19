use scoop_identity::DecodedOptionalSignatureType;
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    BinderListValidationError, CanonicalBinderListV1, DecodedCanonicalBinderListV1,
    DecodedTypeParameterBoundsV1, SignatureTypeReferenceResolver,
};
use crate::MeteredInterfaceResolutionError as Error;
use crate::cross_cone_interface::metered_resolution::charge_name;

impl DecodedCanonicalBinderListV1 {
    pub fn resolve_metered<R: SignatureTypeReferenceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalBinderListV1, Error<BinderListValidationError<E>>> {
        let path = WirePath::root();
        meter
            .charge_nodes(self.binders.len() as u64, &path)
            .map_err(Error::Resource)?;
        meter
            .charge_collection_slots((self.binders.len() as u64).saturating_mul(2), &path)
            .map_err(Error::Resource)?;
        for binder in &self.binders {
            charge_name(&binder.name, 2, meter)?;
            if let DecodedTypeParameterBoundsV1::Nominal(bounds) = &binder.bounds {
                if let DecodedOptionalSignatureType::Present(class) = &bounds.class {
                    class.charge_resolution(meter).map_err(Error::Resource)?;
                }
                meter
                    .charge_collection_slots(bounds.interfaces.values.len() as u64, &path)
                    .map_err(Error::Resource)?;
                for interface in &bounds.interfaces.values {
                    interface
                        .charge_resolution(meter)
                        .map_err(Error::Resource)?;
                }
            }
        }
        self.resolve(resolver).map_err(Error::Value)
    }
}

impl super::DecodedCanonicalSignatureTypesV1 {
    pub fn resolve_metered<R: SignatureTypeReferenceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<super::CanonicalSignatureTypesV1, Error<super::SignatureTypeSetValidationError<E>>>
    {
        meter
            .charge_collection_slots(self.values.len() as u64, &WirePath::root())
            .map_err(Error::Resource)?;
        for value in &self.values {
            value.charge_resolution(meter).map_err(Error::Resource)?;
        }
        self.resolve(resolver).map_err(Error::Value)
    }
}
