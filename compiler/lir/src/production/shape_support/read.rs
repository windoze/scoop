use super::*;
use crate::{LinkDataError, link_data::link_error};
use scoop_identity::PersistentIdResolver;

impl<T> DecodedShapeSupportAvailabilityV1<T> {
    fn read<U>(
        self,
        read: impl FnOnce(T) -> Result<U, LinkDataError>,
    ) -> Result<ShapeSupportAvailabilityV1<U>, LinkDataError> {
        Ok(match self {
            Self::Available(value) => ShapeSupportAvailabilityV1::Available(read(value)?),
            Self::NotApplicable(reason) => ShapeSupportAvailabilityV1::NotApplicable(reason),
        })
    }
}

impl DecodedStrongExactShapeSupportV1 {
    fn read(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<StrongExactShapeSupportV1, LinkDataError> {
        Ok(StrongExactShapeSupportV1::from_artifact(
            identities.resolve(self.nominal).map_err(link_error)?,
            identities.resolve(self.exact).map_err(link_error)?,
            self.layout.read(identities)?,
            self.scan.read(identities)?,
            self.descriptor.read(identities)?,
            self.registration.read(identities)?,
        ))
    }
}

impl DecodedParamFreeShapeSupportRolesV1 {
    pub(crate) fn read(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ParamFreeShapeSupportRolesV1, LinkDataError> {
        Ok(ParamFreeShapeSupportRolesV1::from_artifact(
            ParamFreeShapeSupportRolePartsV1 {
                source_nominal: self
                    .source_nominal
                    .read(|id| identities.resolve(id).map_err(link_error))?,
                value_layout: self.value_layout.read(|value| value.read(identities))?,
                ref_scan: self.ref_scan.read(|value| value.read(identities))?,
                type_descriptor: self.type_descriptor.read(|value| value.read(identities))?,
                type_registration: self
                    .type_registration
                    .read(|value| value.read(identities))?,
                boxed_value: self.boxed_value.read(|value| value.read(identities))?,
                coroutine_step: self.coroutine_step.read(|value| value.read(identities))?,
                coroutine_slot: self.coroutine_slot.read(|value| value.read(identities))?,
            },
        ))
    }
}
