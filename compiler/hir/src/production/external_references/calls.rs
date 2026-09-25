use scoop_identity::{ConcreteExpressionOrigin, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, WirePath};

use super::ExternalHirReferenceProductionError;
use crate::{
    CanonicalDependencyBindingWitnessesV1, CommittedDependencyCallOccurrence, DependencyHirOutput,
    DirectImportedTargetBinding, HirDependencyCallSiteV1,
};

pub(super) enum PendingCallSite<'a> {
    Bound {
        position: crate::concrete::ExecutableExpressionPosition,
        origin: ConcreteExpressionOrigin,
        arguments: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
        binding: &'a DirectImportedTargetBinding,
    },
    Runtime(HirDependencyCallSiteV1),
}

pub(super) fn project<'a, E>(
    output: &DependencyHirOutput,
    call: CommittedDependencyCallOccurrence<'a>,
    meter: &mut BudgetMeter,
) -> Result<PendingCallSite<'a>, ExternalHirReferenceProductionError<E>> {
    use ExternalHirReferenceProductionError as Error;
    let path = WirePath::root();
    let position = call.position();
    let exact = &output.output().local.module().exact_type_identities;
    let mut arguments = Vec::new();
    meter
        .charge_work(2 + call.arguments().len() as u64, &path)
        .map_err(Error::Resource)?;
    meter
        .charge_owned_bytes(
            (call.arguments().len() * std::mem::size_of::<PersistentExactTypeId>()) as u64,
            &path,
        )
        .map_err(Error::Resource)?;
    meter
        .try_reserve_collection_slots(&mut arguments, call.arguments().len(), &path)
        .map_err(Error::Resource)?;
    for argument in call.arguments() {
        arguments.push(
            exact
                .get(argument.ty)
                .ok_or(Error::ExpressionType {
                    position,
                    ty: argument.ty,
                })?
                .id(),
        );
    }
    let result = exact
        .get(call.result_type())
        .ok_or(Error::ExpressionType {
            position,
            ty: call.result_type(),
        })?
        .id();
    let origin = super::origins::project(output, call.origin(), meter)?;
    Ok(PendingCallSite::Bound {
        position,
        origin,
        arguments,
        result,
        binding: call.binding(),
    })
}

impl PendingCallSite<'_> {
    pub(super) fn finish<E>(
        self,
        witnesses: &CanonicalDependencyBindingWitnessesV1,
        meter: &mut BudgetMeter,
    ) -> Result<HirDependencyCallSiteV1, ExternalHirReferenceProductionError<E>> {
        use ExternalHirReferenceProductionError as Error;
        let (position, origin, arguments, result, binding) = match self {
            Self::Bound {
                position,
                origin,
                arguments,
                result,
                binding,
            } => (position, origin, arguments, result, binding),
            Self::Runtime(site) => return Ok(site),
        };
        let path = WirePath::root();
        let mut indices = Vec::new();
        meter
            .charge_owned_bytes(
                (binding.source_count() * std::mem::size_of::<u32>()) as u64,
                &path,
            )
            .map_err(Error::Resource)?;
        meter
            .try_reserve_collection_slots(&mut indices, binding.source_count(), &path)
            .map_err(Error::Resource)?;
        for source in binding.sources() {
            let work = (source.witness().route().hops().len() as u64 + 1)
                .saturating_mul(u64::from(witnesses.witnesses().len().max(1).ilog2()) + 1);
            meter.charge_work(work, &path).map_err(Error::Resource)?;
            let index = witnesses
                .witnesses()
                .binary_search(source.witness().dependency())
                .map_err(|_| Error::MissingCallWitness(position))?;
            indices.push(u32::try_from(index).map_err(|_| Error::MissingCallWitness(position))?);
        }
        indices.sort_unstable();
        indices.dedup();
        HirDependencyCallSiteV1::try_new(position, origin, arguments, result, indices)
            .map_err(Error::CallSite)
    }
}
