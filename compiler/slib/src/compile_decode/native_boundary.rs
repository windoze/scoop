use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use scoop_hir::{NativeBoundaryNominalOwner, NativeBoundaryTypeDefinitionRecord};
use scoop_identity::{
    CallableApplicationKey, CallableArguments, CallableInstantiationOwner,
    CallableMaterializationContext, CallbackApplicationKey, CallbackRegistrationKey,
    CanonicalCAbiLayoutFingerprintRecord, CanonicalCAbiSignatureFingerprintRecord,
    CborIdentityRecord, ExactTypeKey, IdentityLayer, IdentityValidationError,
    InitializationUnitKey, NativeExternalContractRecord, OptionalSignatureType,
    PersistentCallableApplicationId, PersistentCallbackApplicationId, PersistentExactTypeId,
    PersistentInitializationUnitId, SignatureTypeKey, SourceCAbiFunctionSignature,
    SourceCAbiReturn, SourceExternFunctionAbi, SourceNativeExternalContract,
    SourceNativeExternalContractRecord, SourceScoopAbiFunctionSignature, ValidatedIdentityGraph,
};
use scoop_lir::{CallbackBridgeRecord, ConeLirFoundation};
use scoop_mir::CallbackApplicationRecord;
use scoop_wire::{WireError, WirePath};

use crate::ValidatedGraphArtifact;

mod errors;
mod source_closure;
mod source_contracts;
mod target;
pub use errors::NativeBoundaryCompileError;
pub(crate) use target::AbiReplayDependency;
pub use target::NativeBoundaryTargetError;

#[derive(Clone, Copy)]
pub(crate) struct NativeBoundaryFoundationView<'foundation> {
    pub(super) source_contracts: &'foundation [SourceNativeExternalContractRecord],
    pub(super) type_definitions: &'foundation [NativeBoundaryTypeDefinitionRecord],
    pub(super) callback_applications: &'foundation [CallbackApplicationRecord],
    pub(super) native_contracts: &'foundation [NativeExternalContractRecord],
    pub(super) c_abi_signatures: &'foundation [CanonicalCAbiSignatureFingerprintRecord],
    pub(super) c_abi_layouts: &'foundation [CanonicalCAbiLayoutFingerprintRecord],
    pub(super) callback_bridges: &'foundation [CallbackBridgeRecord],
}

impl<'foundation> NativeBoundaryFoundationView<'foundation> {
    pub(crate) fn from_foundations(
        hir: &'foundation scoop_hir::CanonicalHirFoundation,
        mir: &'foundation scoop_mir::CanonicalMirFoundation,
        lir: &'foundation ConeLirFoundation,
    ) -> Self {
        Self {
            source_contracts: hir.source_native_contracts(),
            type_definitions: hir.native_boundary_types(),
            callback_applications: mir.callback_application_records(),
            native_contracts: lir.native_contracts(),
            c_abi_signatures: lir.c_abi_signatures(),
            c_abi_layouts: lir.c_abi_layouts(),
            callback_bridges: lir.callback_bridges(),
        }
    }
}

pub(crate) fn validate_native_boundary_parts(
    graph: &mut ValidatedGraphArtifact<'_>,
    identities: &ValidatedIdentityGraph,
    view: &NativeBoundaryFoundationView<'_>,
) -> Result<(), NativeBoundaryCompileError> {
    source_closure::validate(identities, view)?;
    target::validate_target_normalization(graph, identities, view)
}

pub(crate) fn validate_shared_native_boundary_parts(
    graph: &mut ValidatedGraphArtifact<'_>,
    current: AbiReplayDependency<'_>,
    dependencies: &[AbiReplayDependency<'_>],
    view: &NativeBoundaryFoundationView<'_>,
    materialized_types: &[PersistentExactTypeId],
) -> Result<(), NativeBoundaryCompileError> {
    let source_contracts = source_contracts::resolve(view, dependencies)?;
    let view = NativeBoundaryFoundationView {
        source_contracts: &source_contracts,
        ..*view
    };
    source_closure::validate(current.identities, &view)?;
    target::validate_shared_target_normalization(
        graph,
        current,
        dependencies,
        &view,
        materialized_types,
    )
}

fn records_by_id<I, K>(
    record_sets: impl IntoIterator<Item = Vec<CborIdentityRecord<I, K>>>,

    path: &WirePath,
) -> Result<HashMap<I, Arc<K>>, NativeBoundaryCompileError>
where
    I: scoop_identity::PersistentId + Eq + Hash,
{
    let mut records = HashMap::new();
    for record_set in record_sets {
        for record in record_set {
            if records.contains_key(&record.id()) {
                continue;
            }
            scoop_wire::allocation::try_reserve_map(&mut records, 1, path)
                .map_err(NativeBoundaryCompileError::Resource)?;
            records.insert(record.id(), record.into_shared_key());
        }
    }
    Ok(records)
}

fn index_records<'record, K, V>(
    records: &'record [V],
    key: impl Fn(&V) -> K,

    path: &WirePath,
) -> Result<HashMap<K, &'record V>, NativeBoundaryCompileError>
where
    K: Eq + Hash,
{
    let mut index = HashMap::new();
    scoop_wire::allocation::try_reserve_map(&mut index, records.len(), path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    for record in records {
        index.insert(key(record), record);
    }
    Ok(index)
}
