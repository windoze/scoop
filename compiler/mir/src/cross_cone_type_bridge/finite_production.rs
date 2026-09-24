//! Actual MIR projection of the finite source shape-support materializations.

use super::*;
use crate::{GeneratedExactTypeLocation, SingleConeStrongMirInput, StrongBoxedShapeSupportRoot};

mod representation;

impl CanonicalParamFreeMirTypeExportsV1 {
    /// Produces the finite generated constituent of the complete type table.
    /// The sealed source plan determines membership; unrelated execution
    /// environments are not part of this export surface.
    pub fn from_finite_shape_support(
        input: &SingleConeStrongMirInput,
        sources: &CanonicalParamFreeMirTypeExportsV1,
        identities: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeError> {
        let path = WirePath::root();
        let plan = input.materialization();
        let authority = MirTypeBridgeAuthority {
            identities,
            foundation: input.foundation(),
        };
        let mut records = Vec::new();
        reserve(
            &mut records,
            plan.shape_support().len().saturating_mul(3),
            meter,
        )?;
        for root in plan.shape_support() {
            let exact = root.shape().exact();
            meter
                .charge_work(
                    u64::from(sources.records().len().checked_ilog2().unwrap_or(0)) + 1,
                    &path,
                )
                .map_err(MirTypeBridgeError::Resource)?;
            let source = sources
                .get(exact)
                .ok_or(MirTypeBridgeError::MissingShapeSupportSource { exact })?;
            if source.origin() != &MirTypeOriginV1::SourceNominal(root.shape().source()) {
                return Err(MirTypeBridgeError::ExactOriginMismatch { exact });
            }
            let boxed = match root.boxed() {
                StrongBoxedShapeSupportRoot::Available(boxed) => {
                    Some((boxed, GeneratedNominalKey::BoxedValue { payload: exact }))
                }
                StrongBoxedShapeSupportRoot::ReferenceNominalRequiresNoBox => None,
            };
            let helpers = [
                boxed,
                Some((
                    root.coroutine_step(),
                    GeneratedNominalKey::CoroutineStep { result: exact },
                )),
                Some((
                    root.coroutine_slot(),
                    GeneratedNominalKey::CoroutineSlot { value: exact },
                )),
            ];
            for (helper, role) in helpers.into_iter().flatten() {
                meter
                    .charge_nodes(1, &path)
                    .map_err(MirTypeBridgeError::Resource)?;
                meter
                    .charge_work(1, &path)
                    .map_err(MirTypeBridgeError::Resource)?;
                let (facts, representation, bases) =
                    representation::project(input, source, helper.location(), &role, meter)?;
                records.push(ParamFreeMirTypeExportV1::try_new(
                    authority,
                    helper.exact(),
                    MirTypeOriginV1::GeneratedNominal {
                        nominal: helper.nominal(),
                        role,
                    },
                    facts,
                    representation,
                    bases,
                )?);
            }
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
