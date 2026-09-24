use scoop_identity::{ConcreteExpressionOrigin, EvaluationOrigin, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, WirePath};

use super::ExternalHirReferenceProductionError;
use crate::{
    CanonicalDependencyBindingWitnessesV1, CommittedDependencyCallOccurrence, DependencyHirOutput,
    DirectImportedTargetBinding, HirDependencyCallSiteV1,
};

pub(super) struct PendingCallSite<'a> {
    position: crate::concrete::ExecutableExpressionPosition,
    origin: ConcreteExpressionOrigin,
    arguments: Vec<PersistentExactTypeId>,
    result: PersistentExactTypeId,
    binding: &'a DirectImportedTargetBinding,
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
                .ok_or(Error::CallType {
                    position,
                    ty: argument.ty,
                })?
                .id(),
        );
    }
    let result = exact
        .get(call.result_type())
        .ok_or(Error::CallType {
            position,
            ty: call.result_type(),
        })?
        .id();
    let origin = call.origin();
    let export = output.output().export.module();
    for source in [origin.definition.file, origin.evaluation.file] {
        if let Some(file) = export.source_files.get(source as usize) {
            meter
                .charge_owned_bytes(
                    (file.identity.logical_path().as_str().len() * 2) as u64,
                    &path,
                )
                .map_err(Error::Resource)?;
        }
    }
    let definition = crate::production::project_definition_source(export, origin.definition)
        .map_err(Error::CallOrigin)?;
    let evaluation = crate::production::project_definition_source(
        export,
        crate::DefinitionOrigin {
            provider: origin.evaluation.provider,
            file: origin.evaluation.file,
            span: origin.evaluation.span,
            context: origin.evaluation.context,
        },
    )
    .map_err(Error::CallOrigin)?;
    Ok(PendingCallSite {
        position,
        origin: ConcreteExpressionOrigin::new(
            definition.origin().clone(),
            EvaluationOrigin::at_definition(evaluation.origin()),
        ),
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
        let path = WirePath::root();
        let mut indices = Vec::new();
        meter
            .charge_owned_bytes(
                (self.binding.source_count() * std::mem::size_of::<u32>()) as u64,
                &path,
            )
            .map_err(Error::Resource)?;
        meter
            .try_reserve_collection_slots(&mut indices, self.binding.source_count(), &path)
            .map_err(Error::Resource)?;
        for source in self.binding.sources() {
            let work = (source.witness().route().hops().len() as u64 + 1)
                .saturating_mul(u64::from(witnesses.witnesses().len().max(1).ilog2()) + 1);
            meter.charge_work(work, &path).map_err(Error::Resource)?;
            let index = witnesses
                .witnesses()
                .binary_search(source.witness().dependency())
                .map_err(|_| Error::MissingCallWitness(self.position))?;
            indices
                .push(u32::try_from(index).map_err(|_| Error::MissingCallWitness(self.position))?);
        }
        indices.sort_unstable();
        indices.dedup();
        HirDependencyCallSiteV1::try_new(
            self.position,
            self.origin,
            self.arguments,
            self.result,
            indices,
        )
        .map_err(Error::CallSite)
    }
}
