//! Canonical Scoop ABI replay shared by native and cross-Cone validation.

use std::collections::HashMap;
use std::sync::Arc;

use scoop_hir::OdrFreeHirFoundation;
use scoop_identity::{
    CallableApplicationKey, CanonicalScoopAbiFunctionSignature, ExactCallableSignature,
    ExactTypeKey, GcEffect, IdentityLayer, InitializationUnitKey, PersistentCallableApplicationId,
    PersistentExactTypeId, PersistentInitializationUnitId, ScoopAbiReturn, ValidatedIdentityGraph,
};
use scoop_wire::WirePath;

use super::{AbiNominalDefinition, NativeBoundaryNormalizer, allocate_vec};
use crate::{
    NativeBoundaryCompileError, NativeBoundaryTargetError, ValidatedGraphArtifact,
    compile_decode::native_boundary::records_by_id,
};

mod dependencies;
pub(crate) use dependencies::{AbiReplayDependency, AbiReplayTypes, collect as collect_abi_types};
#[cfg(test)]
mod tests;

pub(super) fn exact_type_records(
    graph: &ValidatedIdentityGraph,
) -> Result<HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>, NativeBoundaryCompileError> {
    records_by_id(
        [
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Hir,
                    &WirePath::root().field(15),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Mir,
                    &WirePath::root().field(1),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Lir,
                    &WirePath::root().field(1),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ],
        &WirePath::root().field(15),
    )
}

pub(crate) fn replay_canonical_scoop_abi<'a>(
    artifact: &mut ValidatedGraphArtifact<'_>,
    current: AbiReplayDependency<'_>,
    dependencies: impl ExactSizeIterator<Item = AbiReplayDependency<'a>>,
    signature: &ExactCallableSignature,
    gc_effect: GcEffect,
) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
    let target = artifact.target_selection().target();

    let mut sources = Vec::new();
    scoop_wire::allocation::try_reserve(&mut sources, dependencies.len(), &WirePath::root())
        .map_err(NativeBoundaryCompileError::Resource)?;
    sources.extend(dependencies);
    replay_canonical_scoop_abi_parts(target, current, &sources, signature, gc_effect)
}

pub(crate) fn replay_canonical_scoop_abi_parts(
    target: scoop_lir::LirTargetProfile,

    current: AbiReplayDependency<'_>,
    dependencies: &[AbiReplayDependency<'_>],
    signature: &ExactCallableSignature,
    gc_effect: GcEffect,
) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
    let types = dependencies::collect(current, dependencies)?;
    types.replay(target, signature, gc_effect)
}

impl AbiReplayTypes<'_> {
    pub(crate) fn replay(
        &self,
        target: scoop_lir::LirTargetProfile,
        signature: &ExactCallableSignature,
        gc_effect: GcEffect,
    ) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
        let callable_applications =
            HashMap::<PersistentCallableApplicationId, Arc<CallableApplicationKey>>::new();
        let initialization_units =
            HashMap::<PersistentInitializationUnitId, Arc<InitializationUnitKey>>::new();
        let mut normalizer = NativeBoundaryNormalizer::new(
            target,
            &self.exact,
            &callable_applications,
            &initialization_units,
            &self.definitions,
        );

        let argument_count =
            signature.parameters().len() + usize::from(signature.receiver().is_present());
        let mut arguments = allocate_vec(argument_count, &WirePath::root().field(3))?;
        for exact in signature
            .receiver()
            .into_option()
            .into_iter()
            .chain(signature.parameters().iter().copied())
        {
            arguments.push(normalizer.scoop_argument(exact)?);
        }
        let result = if normalizer.is_unit(signature.result()) {
            ScoopAbiReturn::unit_void()
        } else {
            normalizer.scoop_return(signature.result())?
        };

        CanonicalScoopAbiFunctionSignature::new(signature.clone(), arguments, result, gc_effect)
            .map_err(NativeBoundaryTargetError::ScoopAbi)
            .map_err(Into::into)
    }
}
