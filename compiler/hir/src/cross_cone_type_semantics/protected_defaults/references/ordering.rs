use std::cmp::Ordering;

use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath, encoded_length};

use super::{ProtectedDefaultReferenceSetResolutionError, ProtectedDefaultReferenceV1};
use crate::{
    DefaultCallableReferenceTargetViewV1, DefaultConstructorRefV1,
    DefaultConstructorReferenceTargetViewV1, DefaultFieldRefV1, DefaultFieldReferenceTargetViewV1,
    ExportDefaultCallableTargetV1, compare_default_signature_reference_targets,
};

pub(super) trait ReferenceTarget: Ord {
    fn compare_metered(
        &self,
        other: &Self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Ordering, WireError>;
}

macro_rules! borrowed_target {
    ($target:ty, $view:ty) => {
        impl ReferenceTarget for $target {
            fn compare_metered(
                &self,
                other: &Self,
                meter: &mut BudgetMeter,
                path: &WirePath,
            ) -> Result<Ordering, WireError> {
                <$view>::from(self).compare_to(other, meter, path)
            }
        }
    };
}
borrowed_target!(
    ExportDefaultCallableTargetV1,
    DefaultCallableReferenceTargetViewV1<'_>
);
borrowed_target!(
    DefaultConstructorRefV1,
    DefaultConstructorReferenceTargetViewV1<'_>
);
borrowed_target!(DefaultFieldRefV1, DefaultFieldReferenceTargetViewV1<'_>);

impl ReferenceTarget for SignatureTypeKey {
    fn compare_metered(
        &self,
        other: &Self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Ordering, WireError> {
        compare_default_signature_reference_targets(self, other, meter, path)
    }
}
macro_rules! identity_target {
    ($target:ty) => {
        impl ReferenceTarget for $target {
            fn compare_metered(
                &self,
                other: &Self,
                meter: &mut BudgetMeter,
                path: &WirePath,
            ) -> Result<Ordering, WireError> {
                meter.charge_work(1, path)?;
                Ok(self.cmp(other))
            }
        }
    };
}
identity_target!(PersistentPropertyId);
identity_target!(PersistentObjectValueId);

pub(super) fn compare_keys<T: ReferenceTarget, E>(
    left: &ProtectedDefaultReferenceV1<T>,
    right: &ProtectedDefaultReferenceV1<T>,
    meter: &mut BudgetMeter,
) -> Result<Ordering, ProtectedDefaultReferenceSetResolutionError<E>> {
    use ProtectedDefaultReferenceSetResolutionError as Error;
    let path = WirePath::root();
    let ordering = left
        .target()
        .compare_metered(right.target(), meter, &path)
        .map_err(Error::Resource)?;
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    let left_bytes = encoded_length(left.definition_origin()).map_err(Error::Encoding)?;
    let right_bytes = encoded_length(right.definition_origin()).map_err(Error::Encoding)?;
    meter
        .charge_work(left_bytes.saturating_add(right_bytes), &path)
        .map_err(Error::Resource)?;
    Ok(left.definition_origin().cmp(right.definition_origin()))
}
