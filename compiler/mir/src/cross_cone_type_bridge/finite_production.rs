//! Actual MIR projection of the finite source shape-support materializations.

use super::*;
use crate::{GeneratedExactTypeLocation, SingleConeStrongMirInput};

mod representation;

impl CanonicalParamFreeMirTypeExportsV1 {
    /// Produces the finite generated constituent of the complete type table.
    /// The sealed source plan determines membership; unrelated execution
    /// environments are not part of this export surface.
    pub fn from_finite_shape_support(
        input: &SingleConeStrongMirInput,
        identities: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeError> {
        let path = WirePath::root();
        let plan = input.materialization();
        let mut required = Vec::new();
        reserve(&mut required, plan.shape_support().len(), meter)?;
        for root in plan.shape_support() {
            required.push(root.shape().exact());
        }
        charge_sort(required.len(), meter)?;
        required.sort_unstable();
        let authority = MirTypeBridgeAuthority {
            identities,
            foundation: input.foundation(),
        };
        let mut records = Vec::new();
        reserve(&mut records, required.len().saturating_mul(3), meter)?;
        let module = input.module();
        for identity in module.meta.generated_exact_types.iter() {
            meter
                .charge_nodes(1, &path)
                .map_err(MirTypeBridgeError::Resource)?;
            meter
                .charge_work(
                    required
                        .len()
                        .checked_ilog2()
                        .map_or(1, |depth| u64::from(depth) + 2),
                    &path,
                )
                .map_err(MirTypeBridgeError::Resource)?;
            let role = identity.nominal_record().key();
            let subject = match role {
                GeneratedNominalKey::BoxedValue { payload } => *payload,
                GeneratedNominalKey::CoroutineStep { result } => *result,
                GeneratedNominalKey::CoroutineSlot { value } => *value,
                GeneratedNominalKey::ObjectBackingClass { .. }
                | GeneratedNominalKey::ClosureEnvironment { .. }
                | GeneratedNominalKey::CallableAdapterEnvironment { .. }
                | GeneratedNominalKey::CoroutineFrame { .. }
                | GeneratedNominalKey::ContinuationAdapterEnvironment { .. } => continue,
            };
            if required.binary_search(&subject).is_err() {
                continue;
            }
            let (facts, representation, bases) =
                representation::project(input, identity.location(), role, meter)?;
            records.push(ParamFreeMirTypeExportV1::try_new(
                authority,
                identity.exact_record().id(),
                MirTypeOriginV1::GeneratedNominal {
                    nominal: identity.nominal_record().id(),
                    role: role.clone(),
                },
                facts,
                representation,
                bases,
            )?);
        }
        charge_sort(records.len(), meter)?;
        Self::try_new(records)
    }
}

fn charge_sort(count: usize, meter: &mut BudgetMeter) -> Result<(), MirTypeBridgeError> {
    let count = count as u64;
    meter
        .charge_work(
            count.saturating_mul(u64::from(count.checked_ilog2().unwrap_or(0)) + 1),
            &WirePath::root(),
        )
        .map_err(MirTypeBridgeError::Resource)
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    meter: &mut BudgetMeter,
) -> Result<(), MirTypeBridgeError> {
    let path = WirePath::root();
    meter
        .charge_owned_bytes(
            (count as u64).saturating_mul(std::mem::size_of::<T>() as u64),
            &path,
        )
        .map_err(MirTypeBridgeError::Resource)?;
    meter
        .try_reserve_collection_slots(values, count, &path)
        .map_err(MirTypeBridgeError::Resource)
}
