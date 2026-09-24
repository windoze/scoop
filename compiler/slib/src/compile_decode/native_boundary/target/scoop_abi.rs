//! Canonical Scoop ABI replay shared by native and cross-Cone validation.

use std::collections::HashMap;
use std::sync::Arc;

use scoop_hir::OdrFreeHirFoundation;
use scoop_identity::{
    CallableApplicationKey, CanonicalScoopAbiFunctionSignature, ExactCallableSignature,
    ExactTypeKey, GcEffect, IdentityLayer, InitializationUnitKey, PersistentCallableApplicationId,
    PersistentExactTypeId, PersistentInitializationUnitId, ScoopAbiReturn, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{AbiNominalDefinition, NativeBoundaryNormalizer, metered_vec};
use crate::{
    NativeBoundaryCompileError, NativeBoundaryTargetError, ValidatedGraphArtifact,
    compile_decode::native_boundary::records_by_id,
};

mod dependencies;
pub(crate) use dependencies::AbiReplayDependency;
#[cfg(test)]
mod tests;

pub(super) fn exact_type_records(
    graph: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>, NativeBoundaryCompileError> {
    records_by_id(
        [
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Hir,
                    meter,
                    &WirePath::root().field(15),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Mir,
                    meter,
                    &WirePath::root().field(1),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Lir,
                    meter,
                    &WirePath::root().field(1),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ],
        meter,
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
    let meter = artifact.envelope.meter_mut();
    let mut sources = Vec::new();
    meter
        .try_reserve_collection_slots(&mut sources, dependencies.len(), &WirePath::root())
        .map_err(NativeBoundaryCompileError::Resource)?;
    sources.extend(dependencies);
    replay_canonical_scoop_abi_parts(target, meter, current, &sources, signature, gc_effect)
}

pub(crate) fn replay_canonical_scoop_abi_parts(
    target: scoop_lir::LirTargetProfile,
    meter: &mut BudgetMeter,
    current: AbiReplayDependency<'_>,
    dependencies: &[AbiReplayDependency<'_>],
    signature: &ExactCallableSignature,
    gc_effect: GcEffect,
) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
    let types = dependencies::collect(current, dependencies, meter)?;
    let callable_applications =
        HashMap::<PersistentCallableApplicationId, Arc<CallableApplicationKey>>::new();
    let initialization_units =
        HashMap::<PersistentInitializationUnitId, Arc<InitializationUnitKey>>::new();
    let mut normalizer = NativeBoundaryNormalizer::new(
        target,
        meter,
        &types.exact,
        &callable_applications,
        &initialization_units,
        &types.definitions,
    );

    let argument_count =
        signature.parameters().len() + usize::from(signature.receiver().is_present());
    let mut arguments = metered_vec(normalizer.meter, argument_count, &WirePath::root().field(3))?;
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
    let parameters = signature.parameters().len() as u64;
    let path = WirePath::root().field(3);
    normalizer
        .meter
        .charge_collection_slots(parameters, &path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    normalizer
        .meter
        .charge_owned_bytes(
            parameters.saturating_mul(std::mem::size_of::<PersistentExactTypeId>() as u64),
            &path,
        )
        .map_err(NativeBoundaryCompileError::Resource)?;
    CanonicalScoopAbiFunctionSignature::new(signature.clone(), arguments, result, gc_effect)
        .map_err(NativeBoundaryTargetError::ScoopAbi)
        .map_err(Into::into)
}
