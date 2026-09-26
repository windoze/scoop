use scoop_identity::{ConcreteExpressionOrigin, PersistentExactTypeId};
use scoop_wire::WirePath;

use super::ExternalHirReferenceProductionError;
use crate::{
    CanonicalDependencyBindingWitnessesV1, CommittedDependencyCallOccurrence, DependencyHirOutput,
    DirectImportedTargetBinding, HirDependencyCallReasonV1, HirDependencyCallSiteV1,
};

pub(super) struct PendingCallSite<'a> {
    position: crate::concrete::ExecutableExpressionPosition,
    origin: ConcreteExpressionOrigin,
    arguments: Vec<PersistentExactTypeId>,
    result: PersistentExactTypeId,
    receiver: crate::SourceCallReceiver<PersistentExactTypeId>,
    binding: Option<&'a DirectImportedTargetBinding>,
}

pub(super) fn project<'a, E>(
    output: &DependencyHirOutput,
    call: CommittedDependencyCallOccurrence<'a>,
) -> Result<PendingCallSite<'a>, ExternalHirReferenceProductionError<E>> {
    use ExternalHirReferenceProductionError as Error;
    let path = WirePath::root();
    let position = call.position();
    let exact = &output.output().local.module().exact_type_identities;
    let mut arguments = Vec::new();

    scoop_wire::allocation::try_reserve(&mut arguments, call.arguments().len(), &path)
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
    let origin = super::origins::project(output, call.origin())?;
    let receiver = call.receiver().try_map(|ty| {
        exact
            .get(ty)
            .map(|identity| identity.id())
            .ok_or(Error::ExpressionType { position, ty })
    })?;
    Ok(PendingCallSite {
        position,
        origin,
        arguments,
        result,
        receiver,
        binding: call.binding(),
    })
}

impl PendingCallSite<'_> {
    pub(super) fn finish<E>(
        self,
        witnesses: &CanonicalDependencyBindingWitnessesV1,
    ) -> Result<HirDependencyCallSiteV1, ExternalHirReferenceProductionError<E>> {
        use ExternalHirReferenceProductionError as Error;
        let Self {
            position,
            origin,
            arguments,
            result,
            receiver,
            binding,
        } = self;
        let Some(binding) = binding else {
            return HirDependencyCallSiteV1::try_new_with_reason(
                position,
                origin,
                arguments,
                result,
                HirDependencyCallReasonV1::SourceDeclaration,
                receiver,
            )
            .map_err(Error::CallSite);
        };
        let path = WirePath::root();
        let mut indices = Vec::new();

        scoop_wire::allocation::try_reserve(&mut indices, binding.source_count(), &path)
            .map_err(Error::Resource)?;
        for source in binding.sources() {
            let index = witnesses
                .witnesses()
                .binary_search(source.witness().dependency())
                .map_err(|_| Error::MissingCallWitness(position))?;
            indices.push(u32::try_from(index).map_err(|_| Error::MissingCallWitness(position))?);
        }
        indices.sort_unstable();
        indices.dedup();
        HirDependencyCallSiteV1::try_new(position, origin, arguments, result, indices, receiver)
            .map_err(Error::CallSite)
    }
}
