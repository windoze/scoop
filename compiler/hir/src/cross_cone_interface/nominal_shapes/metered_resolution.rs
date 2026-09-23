use super::*;
use crate::MeteredInterfaceResolutionError as Error;
use scoop_wire::{BudgetMeter, WirePath};

impl DecodedNominalSourceShapeV1 {
    pub fn resolve_metered<R: NominalSourceShapeResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<NominalSourceShapeV1, Error<NominalSourceShapeResolutionError<E>>> {
        let path = WirePath::root();
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        match &self {
            Self::Class(fields) | Self::Struct { fields, .. } | Self::Object { fields, .. } => {
                meter
                    .charge_collection_slots((fields.len() as u64).saturating_mul(2), &path)
                    .map_err(Error::Resource)?;
                for field in fields {
                    field
                        .value_type
                        .charge_resolution(meter)
                        .map_err(Error::Resource)?;
                }
            }
            Self::Enum(variants) => {
                meter
                    .charge_collection_slots((variants.len() as u64).saturating_mul(2), &path)
                    .map_err(Error::Resource)?;
                for variant in variants {
                    meter.charge_nodes(1, &path).map_err(Error::Resource)?;
                    meter
                        .charge_collection_slots(
                            (variant.fields.len() as u64).saturating_mul(2),
                            &path,
                        )
                        .map_err(Error::Resource)?;
                    for field in &variant.fields {
                        field
                            .value_type
                            .charge_resolution(meter)
                            .map_err(Error::Resource)?;
                    }
                }
            }
            Self::Interface | Self::Intrinsic(_) => {
                return self.resolve(resolver).map_err(Error::Value);
            }
        }
        self.resolve(resolver).map_err(Error::Value)
    }
}
