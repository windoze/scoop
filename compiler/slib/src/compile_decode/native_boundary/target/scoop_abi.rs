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

use super::{NativeBoundaryNormalizer, metered_vec};
use crate::{
    NativeBoundaryCompileError, NativeBoundaryTargetError, ValidatedGraphArtifact,
    compile_decode::native_boundary::{index_records, records_by_id},
};

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

pub(crate) fn replay_canonical_scoop_abi(
    artifact: &mut ValidatedGraphArtifact<'_>,
    identities: &ValidatedIdentityGraph,
    hir_foundation: &OdrFreeHirFoundation,
    signature: &ExactCallableSignature,
    gc_effect: GcEffect,
) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
    let target = artifact.target_selection().target();
    let meter = artifact.envelope.meter_mut();
    let exact_types = exact_type_records(identities, meter)?;
    let definitions = index_records(
        hir_foundation.native_boundary_types(),
        scoop_hir::NativeBoundaryTypeDefinitionRecord::owner,
        meter,
        &WirePath::root().field(30),
    )?;
    let callable_applications =
        HashMap::<PersistentCallableApplicationId, Arc<CallableApplicationKey>>::new();
    let initialization_units =
        HashMap::<PersistentInitializationUnitId, Arc<InitializationUnitKey>>::new();
    let mut normalizer = NativeBoundaryNormalizer::new(
        target,
        meter,
        &exact_types,
        &callable_applications,
        &initialization_units,
        &definitions,
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
    CanonicalScoopAbiFunctionSignature::new(signature.clone(), arguments, result, gc_effect)
        .map_err(NativeBoundaryTargetError::ScoopAbi)
        .map_err(Into::into)
}
