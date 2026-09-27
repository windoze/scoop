use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use scoop_hir::{
    NativeBoundaryNominalOwner, NativeBoundaryTypeDefinitionRecord, OdrFreeHirFoundation,
};
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
use scoop_lir::{CallbackBridgeRecord, OdrFreeLirFoundation};
use scoop_mir::{CallbackApplicationRecord, OdrFreeMirFoundation};
use scoop_wire::{WireError, WirePath};

use crate::ValidatedGraphArtifact;

mod errors;
mod target;
pub use errors::NativeBoundaryCompileError;
pub use target::NativeBoundaryTargetError;
pub(crate) use target::{AbiReplayDependency, collect_abi_types, replay_canonical_scoop_abi};

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
    pub(crate) fn from_odr_free(
        hir: &'foundation OdrFreeHirFoundation,
        mir: &'foundation OdrFreeMirFoundation,
        lir: &'foundation OdrFreeLirFoundation,
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
    validate_source_closure(identities, view)?;
    target::validate_target_normalization(graph, identities, view)
}

pub(crate) fn validate_shared_native_boundary_parts(
    graph: &mut ValidatedGraphArtifact<'_>,
    current: AbiReplayDependency<'_>,
    dependencies: &[AbiReplayDependency<'_>],
    view: &NativeBoundaryFoundationView<'_>,
    materialized_types: &[PersistentExactTypeId],
) -> Result<(), NativeBoundaryCompileError> {
    validate_source_closure(current.identities, view)?;
    target::validate_shared_target_normalization(
        graph,
        current,
        dependencies,
        view,
        materialized_types,
    )
}

fn validate_source_closure(
    graph: &ValidatedIdentityGraph,
    view: &NativeBoundaryFoundationView<'_>,
) -> Result<(), NativeBoundaryCompileError> {
    let exact_types = records_by_id(
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
    )?;
    let callable_applications = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentCallableApplicationId, CallableApplicationKey>(
                    IdentityLayer::Hir,
                    &WirePath::root().field(17),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        &WirePath::root().field(17),
    )?;
    let initialization_units = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentInitializationUnitId, InitializationUnitKey>(
                    IdentityLayer::Hir,
                    &WirePath::root().field(21),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        &WirePath::root().field(21),
    )?;
    let callback_registrations = graph
        .records::<scoop_identity::PersistentCallbackRegistrationId, CallbackRegistrationKey>(
            IdentityLayer::Hir,
            &WirePath::root().field(25),
        )
        .map_err(NativeBoundaryCompileError::Identity)?;
    let callback_applications = graph
        .records::<PersistentCallbackApplicationId, CallbackApplicationKey>(
            IdentityLayer::Mir,
            &WirePath::root().field(9),
        )
        .map_err(NativeBoundaryCompileError::Identity)?;
    let definitions = index_records(
        view.type_definitions,
        scoop_hir::NativeBoundaryTypeDefinitionRecord::owner,
        &WirePath::root().field(34),
    )?;

    let mut closure = SourceClosureState::default();
    for source in view.source_contracts {
        collect_source_contract(source.contract(), &mut closure, &WirePath::root().field(26))?;
    }
    for registration in callback_registrations {
        let path = WirePath::root().field(25);
        collect_c_signature(registration.key().source_signature(), &mut closure, &path)?;
        collect_optional_signature_type(
            registration.key().managed_signature().receiver(),
            &mut closure,
            &path,
        )?;
        for parameter in registration.key().managed_signature().parameters() {
            collect_signature_type(parameter, &mut closure, &path)?;
        }
        collect_signature_type(
            registration.key().managed_signature().result(),
            &mut closure,
            &path,
        )?;
    }

    for application in callback_applications {
        collect_materialization_context(
            application.key().context(),
            &exact_types,
            &callable_applications,
            &initialization_units,
            &mut closure,
        )?;
    }

    let mut next_owner = 0;
    while let Some(owner) = closure.required_owners.get(next_owner).copied() {
        next_owner += 1;
        let Some(definition) = definitions.get(&owner) else {
            continue;
        };
        match definition.shape() {
            scoop_hir::NativeBoundaryNominalShape::Reference
            | scoop_hir::NativeBoundaryNominalShape::Intrinsic(_) => continue,
            scoop_hir::NativeBoundaryNominalShape::Struct { fields, .. } => {
                for field in fields {
                    collect_signature_type(field.ty(), &mut closure, &WirePath::root().field(34))?;
                }
            }
            scoop_hir::NativeBoundaryNominalShape::Enum { variants } => {
                for variant in variants {
                    for field in variant.fields() {
                        collect_signature_type(
                            field.ty(),
                            &mut closure,
                            &WirePath::root().field(34),
                        )?;
                    }
                }
            }
        }
    }
    if let Some(owner) = closure
        .required_owners
        .iter()
        .copied()
        .filter(|owner| !definitions.contains_key(owner))
        .min()
    {
        return Err(NativeBoundaryCompileError::ClosureRequired { owner });
    }
    if let Some(owner) = definitions
        .keys()
        .copied()
        .filter(|owner| !closure.required_owner_set.contains(owner))
        .min()
    {
        return Err(NativeBoundaryCompileError::UnrelatedDefinition { owner });
    }
    Ok(())
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

#[derive(Default)]
struct SourceClosureState {
    required_owners: Vec<NativeBoundaryNominalOwner>,
    required_owner_set: HashSet<NativeBoundaryNominalOwner>,
    visited_callables: HashSet<PersistentCallableApplicationId>,
    visited_initializations: HashSet<PersistentInitializationUnitId>,
    visited_exact_types: HashSet<PersistentExactTypeId>,
}

impl SourceClosureState {
    fn require_owner(
        &mut self,
        owner: NativeBoundaryNominalOwner,
        path: &WirePath,
    ) -> Result<(), NativeBoundaryCompileError> {
        if self.required_owner_set.contains(&owner) {
            return Ok(());
        }
        scoop_wire::allocation::try_reserve_set(&mut self.required_owner_set, 1, path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        scoop_wire::allocation::try_reserve(&mut self.required_owners, 1, path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        self.required_owner_set.insert(owner);
        self.required_owners.push(owner);
        Ok(())
    }
}

fn collect_source_contract(
    contract: &SourceNativeExternalContract,
    state: &mut SourceClosureState,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    match contract {
        SourceNativeExternalContract::Function { abi, .. } => match abi {
            SourceExternFunctionAbi::C(signature) => collect_c_signature(signature, state, path)?,
            SourceExternFunctionAbi::Scoop { signature, .. } => {
                collect_scoop_signature(signature, state, path)?;
            }
        },
        SourceNativeExternalContract::ReadOnlyData { storage, .. }
        | SourceNativeExternalContract::MutableData { storage, .. }
        | SourceNativeExternalContract::ReadOnlyTls { storage, .. }
        | SourceNativeExternalContract::MutableTls { storage, .. } => {
            collect_signature_type(storage, state, path)?;
        }
    }
    Ok(())
}

fn collect_c_signature(
    signature: &SourceCAbiFunctionSignature,
    state: &mut SourceClosureState,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    for parameter in signature.parameters() {
        collect_signature_type(parameter, state, path)?;
    }
    if let SourceCAbiReturn::Value(result) = signature.result() {
        collect_signature_type(result, state, path)?;
    }
    Ok(())
}

fn collect_scoop_signature(
    signature: &SourceScoopAbiFunctionSignature,
    state: &mut SourceClosureState,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    for parameter in signature.parameters() {
        collect_signature_type(parameter, state, path)?;
    }
    collect_signature_type(signature.result(), state, path)
}

fn collect_optional_signature_type(
    ty: &OptionalSignatureType,
    state: &mut SourceClosureState,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    if let OptionalSignatureType::Present(ty) = ty {
        collect_signature_type(ty, state, path)?;
    }
    Ok(())
}

fn collect_signature_type(
    ty: &SignatureTypeKey,
    state: &mut SourceClosureState,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    let mut pending = Vec::new();
    scoop_wire::allocation::try_reserve(&mut pending, 1, path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    pending.push(ty);
    while let Some(ty) = pending.pop() {
        let children = match ty {
            SignatureTypeKey::Nominal(owner) => {
                state.require_owner(NativeBoundaryNominalOwner::Concrete(*owner), path)?;
                continue;
            }
            SignatureTypeKey::NominalApplication { origin, arguments } => {
                state.require_owner(NativeBoundaryNominalOwner::GenericTemplate(*origin), path)?;
                arguments.as_slice()
            }
            SignatureTypeKey::Tuple(elements) => elements.as_slice(),
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                scoop_wire::allocation::try_reserve(&mut pending, 1, path)
                    .map_err(NativeBoundaryCompileError::Resource)?;
                pending.push(result.as_ref());
                parameters.as_slice()
            }
            SignatureTypeKey::RawPointer(pointee) => std::slice::from_ref(pointee.as_ref()),
            SignatureTypeKey::Binder { .. } => continue,
        };
        scoop_wire::allocation::try_reserve(&mut pending, children.len(), path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        pending.extend(children.iter());
    }
    Ok(())
}

fn collect_materialization_context(
    context: CallableMaterializationContext,
    exact_types: &HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    callable_applications: &HashMap<PersistentCallableApplicationId, Arc<CallableApplicationKey>>,
    initialization_units: &HashMap<PersistentInitializationUnitId, Arc<InitializationUnitKey>>,
    state: &mut SourceClosureState,
) -> Result<(), NativeBoundaryCompileError> {
    match context {
        CallableMaterializationContext::NoSubstitution => Ok(()),
        CallableMaterializationContext::Application(application) => collect_callable_application(
            application,
            exact_types,
            callable_applications,
            initialization_units,
            state,
        ),
        CallableMaterializationContext::InitializationApplication(unit) => {
            collect_initialization_application(unit, exact_types, initialization_units, state)
        }
    }
}

fn collect_callable_application(
    mut application: PersistentCallableApplicationId,
    exact_types: &HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    callable_applications: &HashMap<PersistentCallableApplicationId, Arc<CallableApplicationKey>>,
    initialization_units: &HashMap<PersistentInitializationUnitId, Arc<InitializationUnitKey>>,
    state: &mut SourceClosureState,
) -> Result<(), NativeBoundaryCompileError> {
    let path = WirePath::root().field(17);
    // The identity graph has already checked references and rejected cycles.
    while !state.visited_callables.contains(&application) {
        let key = callable_applications
            .get(&application)
            .ok_or(NativeBoundaryCompileError::MissingCallableApplication { application })?;
        scoop_wire::allocation::try_reserve_set(&mut state.visited_callables, 1, &path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        state.visited_callables.insert(application);
        if let CallableArguments::Arguments(arguments) = key.callable_arguments() {
            for argument in arguments.as_slice() {
                collect_exact_type(*argument, exact_types, state)?;
            }
        }
        match key.instantiation_owner() {
            CallableInstantiationOwner::NoOwner => break,
            CallableInstantiationOwner::ExactNominalOwner(owner) => {
                collect_exact_type(owner, exact_types, state)?;
                break;
            }
            CallableInstantiationOwner::EnclosingCallableApplication(enclosing) => {
                application = enclosing;
            }
            CallableInstantiationOwner::EnclosingInitializationApplication(unit) => {
                collect_initialization_application(unit, exact_types, initialization_units, state)?;
                break;
            }
        }
    }
    Ok(())
}

fn collect_initialization_application(
    unit: PersistentInitializationUnitId,
    exact_types: &HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    initialization_units: &HashMap<PersistentInitializationUnitId, Arc<InitializationUnitKey>>,
    state: &mut SourceClosureState,
) -> Result<(), NativeBoundaryCompileError> {
    if state.visited_initializations.contains(&unit) {
        return Ok(());
    }
    let path = WirePath::root().field(21);
    let key = initialization_units
        .get(&unit)
        .ok_or(NativeBoundaryCompileError::MissingInitializationUnit { unit })?;
    scoop_wire::allocation::try_reserve_set(&mut state.visited_initializations, 1, &path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    state.visited_initializations.insert(unit);
    if let InitializationUnitKey::GenericDelegatedExtensionApplication {
        receiver_arguments, ..
    } = key.as_ref()
    {
        for argument in receiver_arguments.as_slice() {
            collect_exact_type(*argument, exact_types, state)?;
        }
    }
    Ok(())
}

fn collect_exact_type(
    exact: PersistentExactTypeId,
    exact_types: &HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    state: &mut SourceClosureState,
) -> Result<(), NativeBoundaryCompileError> {
    let path = WirePath::root().field(15);
    let mut pending = Vec::new();
    scoop_wire::allocation::try_reserve(&mut pending, 1, &path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    pending.push(exact);
    while let Some(exact) = pending.pop() {
        if state.visited_exact_types.contains(&exact) {
            continue;
        }
        let key = exact_types
            .get(&exact)
            .ok_or(NativeBoundaryCompileError::MissingExactType { exact })?;
        scoop_wire::allocation::try_reserve_set(&mut state.visited_exact_types, 1, &path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        state.visited_exact_types.insert(exact);
        let children = match key.as_ref() {
            ExactTypeKey::Nominal(owner) => {
                state.require_owner(NativeBoundaryNominalOwner::Concrete(*owner), &path)?;
                continue;
            }
            ExactTypeKey::NominalApplication { origin, arguments } => {
                state.require_owner(NativeBoundaryNominalOwner::GenericTemplate(*origin), &path)?;
                arguments.as_slice()
            }
            ExactTypeKey::Tuple(elements) => elements.as_slice(),
            ExactTypeKey::Function {
                parameters, result, ..
            }
            | ExactTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                scoop_wire::allocation::try_reserve(&mut pending, 1, &path)
                    .map_err(NativeBoundaryCompileError::Resource)?;
                pending.push(*result);
                parameters.as_slice()
            }
            ExactTypeKey::RawPointer(pointee) => std::slice::from_ref(pointee),
        };
        scoop_wire::allocation::try_reserve(&mut pending, children.len(), &path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        pending.extend_from_slice(children);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_identity::CoreBuiltinNominal;

    use super::*;

    #[test]
    fn canonical_key_index_reuses_identity_graph_allocations() {
        let owner = CoreBuiltinNominal::Unit.identity_record().id();
        let record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap();
        let id = record.id();
        let expected = record.clone().into_shared_key();

        let indexed = records_by_id(std::iter::once(vec![record]), &WirePath::root()).unwrap();

        assert!(Arc::ptr_eq(indexed.get(&id).unwrap(), &expected));
    }
}
