use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

use scoop_hir::NativeBoundaryNominalOwner;
use scoop_identity::{
    CallableApplicationKey, CallableArguments, CallableInstantiationOwner,
    CallableMaterializationContext, CallbackApplicationKey, CallbackRegistrationKey,
    CborIdentityRecord, ExactTypeKey, IdentityLayer, IdentityValidationError,
    InitializationUnitKey, OptionalSignatureType, PersistentCallableApplicationId,
    PersistentCallbackApplicationId, PersistentExactTypeId, PersistentInitializationUnitId,
    SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceExternFunctionAbi,
    SourceNativeExternalContract, SourceScoopAbiFunctionSignature,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::StructurallyValidatedFoundations;

mod target;
pub use target::{NativeBoundaryTargetError, NativeBoundaryValidatedFoundations};

/// Structurally valid foundations whose native-boundary source witnesses are
/// exactly the transitive nominal closure required by externs and callbacks.
/// Target-specific ABI normalization remains a separate proof.
pub struct NativeBoundarySourceValidatedFoundations<'input> {
    pub(super) foundations: StructurallyValidatedFoundations<'input>,
}

impl<'input> StructurallyValidatedFoundations<'input> {
    pub fn validate_native_boundary_source(
        mut self,
    ) -> Result<NativeBoundarySourceValidatedFoundations<'input>, NativeBoundaryCompileError> {
        validate_source_closure(&mut self)?;
        Ok(NativeBoundarySourceValidatedFoundations { foundations: self })
    }
}

impl NativeBoundarySourceValidatedFoundations<'_> {
    pub const fn foundations(&self) -> &StructurallyValidatedFoundations<'_> {
        &self.foundations
    }
}

fn validate_source_closure(
    foundations: &mut StructurallyValidatedFoundations<'_>,
) -> Result<(), NativeBoundaryCompileError> {
    let graph = &foundations.identities;
    let meter = foundations.graph.envelope.meter_mut();
    let exact_types = records_by_id(
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
    )?;
    let callable_applications = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentCallableApplicationId, CallableApplicationKey>(
                    IdentityLayer::Hir,
                    meter,
                    &WirePath::root().field(17),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        meter,
        &WirePath::root().field(17),
    )?;
    let initialization_units = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentInitializationUnitId, InitializationUnitKey>(
                    IdentityLayer::Hir,
                    meter,
                    &WirePath::root().field(21),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        meter,
        &WirePath::root().field(21),
    )?;
    let callback_registrations = graph
        .records::<scoop_identity::PersistentCallbackRegistrationId, CallbackRegistrationKey>(
            IdentityLayer::Hir,
            meter,
            &WirePath::root().field(25),
        )
        .map_err(NativeBoundaryCompileError::Identity)?;
    let callback_applications = graph
        .records::<PersistentCallbackApplicationId, CallbackApplicationKey>(
            IdentityLayer::Mir,
            meter,
            &WirePath::root().field(9),
        )
        .map_err(NativeBoundaryCompileError::Identity)?;
    let definitions = index_records(
        foundations.hir.native_boundary_types(),
        scoop_hir::NativeBoundaryTypeDefinitionRecord::owner,
        meter,
        &WirePath::root().field(30),
    )?;

    let mut closure = SourceClosureState::new(meter);
    for source in foundations.hir.source_native_contracts() {
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
            collect_signature_type(parameter, &mut closure, &path, 1)?;
        }
        collect_signature_type(
            registration.key().managed_signature().result(),
            &mut closure,
            &path,
            1,
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
            scoop_hir::NativeBoundaryNominalShape::Reference => {}
            scoop_hir::NativeBoundaryNominalShape::Struct { fields, .. } => {
                for field in fields {
                    collect_signature_type(
                        field.ty(),
                        &mut closure,
                        &WirePath::root().field(30),
                        1,
                    )?;
                }
            }
            scoop_hir::NativeBoundaryNominalShape::Enum { variants } => {
                for variant in variants {
                    for field in variant.fields() {
                        collect_signature_type(
                            field.ty(),
                            &mut closure,
                            &WirePath::root().field(30),
                            1,
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
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<HashMap<I, K>, NativeBoundaryCompileError>
where
    I: scoop_identity::PersistentId + Eq + Hash,
    K: Clone,
{
    let mut records = HashMap::new();
    for record_set in record_sets {
        for record in record_set {
            if records.contains_key(&record.id()) {
                continue;
            }
            meter
                .try_reserve_map_slots(&mut records, 1, path)
                .map_err(NativeBoundaryCompileError::Resource)?;
            records.insert(record.id(), record.into_key());
        }
    }
    Ok(records)
}

fn index_records<'record, K, V>(
    records: &'record [V],
    key: impl Fn(&V) -> K,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<HashMap<K, &'record V>, NativeBoundaryCompileError>
where
    K: Eq + Hash,
{
    let mut index = HashMap::new();
    meter
        .try_reserve_map_slots(&mut index, records.len(), path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    for record in records {
        index.insert(key(record), record);
    }
    Ok(index)
}

struct SourceClosureState<'meter> {
    meter: &'meter mut BudgetMeter,
    required_owners: Vec<NativeBoundaryNominalOwner>,
    required_owner_set: HashSet<NativeBoundaryNominalOwner>,
    callable_heights: HashMap<PersistentCallableApplicationId, u64>,
    initialization_heights: HashMap<PersistentInitializationUnitId, u64>,
    exact_heights: HashMap<PersistentExactTypeId, u64>,
}

impl<'meter> SourceClosureState<'meter> {
    fn new(meter: &'meter mut BudgetMeter) -> Self {
        Self {
            meter,
            required_owners: Vec::new(),
            required_owner_set: HashSet::new(),
            callable_heights: HashMap::new(),
            initialization_heights: HashMap::new(),
            exact_heights: HashMap::new(),
        }
    }

    fn require_owner(
        &mut self,
        owner: NativeBoundaryNominalOwner,
        path: &WirePath,
    ) -> Result<(), NativeBoundaryCompileError> {
        if self.required_owner_set.contains(&owner) {
            return Ok(());
        }
        self.meter
            .try_reserve_set_slots(&mut self.required_owner_set, 1, path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        self.meter
            .try_reserve_collection_slots(&mut self.required_owners, 1, path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        self.required_owner_set.insert(owner);
        self.required_owners.push(owner);
        Ok(())
    }

    fn cache_callable_height(
        &mut self,
        application: PersistentCallableApplicationId,
        height: u64,
        path: &WirePath,
    ) -> Result<(), NativeBoundaryCompileError> {
        Self::cache_height(
            self.meter,
            &mut self.callable_heights,
            application,
            height,
            path,
        )
    }

    fn cache_initialization_height(
        &mut self,
        unit: PersistentInitializationUnitId,
        height: u64,
        path: &WirePath,
    ) -> Result<(), NativeBoundaryCompileError> {
        Self::cache_height(
            self.meter,
            &mut self.initialization_heights,
            unit,
            height,
            path,
        )
    }

    fn cache_exact_height(
        &mut self,
        exact: PersistentExactTypeId,
        height: u64,
        path: &WirePath,
    ) -> Result<(), NativeBoundaryCompileError> {
        Self::cache_height(self.meter, &mut self.exact_heights, exact, height, path)
    }

    fn cache_height<K>(
        meter: &mut BudgetMeter,
        heights: &mut HashMap<K, u64>,
        key: K,
        height: u64,
        path: &WirePath,
    ) -> Result<(), NativeBoundaryCompileError>
    where
        K: Copy + Eq + Hash,
    {
        meter
            .try_reserve_map_slots(heights, 1, path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        heights.insert(key, height);
        Ok(())
    }

    fn charge_relation(&mut self, path: &WirePath) -> Result<(), NativeBoundaryCompileError> {
        self.meter
            .charge_edges(1, path)
            .map_err(NativeBoundaryCompileError::Resource)
    }

    fn check_depth(&self, depth: u64, path: &WirePath) -> Result<(), NativeBoundaryCompileError> {
        self.meter
            .check_semantic_depth(depth, path)
            .map_err(NativeBoundaryCompileError::Resource)
    }

    fn check_subtree_depth(
        &self,
        depth: u64,
        height: u64,
        path: &WirePath,
    ) -> Result<(), NativeBoundaryCompileError> {
        let deepest = depth.checked_add(height.saturating_sub(1)).ok_or_else(|| {
            NativeBoundaryCompileError::Resource(WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })?;
        self.check_depth(deepest, path)
    }
}

fn collect_source_contract(
    contract: &SourceNativeExternalContract,
    state: &mut SourceClosureState<'_>,
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
            collect_signature_type(storage, state, path, 1)?;
        }
    }
    Ok(())
}

fn collect_c_signature(
    signature: &SourceCAbiFunctionSignature,
    state: &mut SourceClosureState<'_>,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    for parameter in signature.parameters() {
        collect_signature_type(parameter, state, path, 1)?;
    }
    if let SourceCAbiReturn::Value(result) = signature.result() {
        collect_signature_type(result, state, path, 1)?;
    }
    Ok(())
}

fn collect_scoop_signature(
    signature: &SourceScoopAbiFunctionSignature,
    state: &mut SourceClosureState<'_>,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    for parameter in signature.parameters() {
        collect_signature_type(parameter, state, path, 1)?;
    }
    collect_signature_type(signature.result(), state, path, 1)
}

fn collect_optional_signature_type(
    ty: &OptionalSignatureType,
    state: &mut SourceClosureState<'_>,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    if let OptionalSignatureType::Present(ty) = ty {
        collect_signature_type(ty, state, path, 1)?;
    }
    Ok(())
}

fn collect_signature_type(
    ty: &SignatureTypeKey,
    state: &mut SourceClosureState<'_>,
    path: &WirePath,
    depth: u64,
) -> Result<(), NativeBoundaryCompileError> {
    state.check_depth(depth, path)?;
    match ty {
        SignatureTypeKey::Nominal(owner) => {
            state.charge_relation(path)?;
            state.require_owner(NativeBoundaryNominalOwner::Concrete(*owner), path)?;
        }
        SignatureTypeKey::NominalApplication { origin, arguments } => {
            state.charge_relation(path)?;
            state.require_owner(NativeBoundaryNominalOwner::GenericTemplate(*origin), path)?;
            for argument in arguments.as_slice() {
                collect_signature_type(argument, state, path, depth + 1)?;
            }
        }
        SignatureTypeKey::Tuple(elements) => {
            for element in elements.as_slice() {
                collect_signature_type(element, state, path, depth + 1)?;
            }
        }
        SignatureTypeKey::Function {
            parameters, result, ..
        }
        | SignatureTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            for parameter in parameters {
                collect_signature_type(parameter, state, path, depth + 1)?;
            }
            collect_signature_type(result, state, path, depth + 1)?;
        }
        SignatureTypeKey::RawPointer(pointee) => {
            collect_signature_type(pointee, state, path, depth + 1)?;
        }
        SignatureTypeKey::Binder { .. } => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_materialization_context(
    context: CallableMaterializationContext,
    exact_types: &HashMap<PersistentExactTypeId, ExactTypeKey>,
    callable_applications: &HashMap<PersistentCallableApplicationId, CallableApplicationKey>,
    initialization_units: &HashMap<PersistentInitializationUnitId, InitializationUnitKey>,
    state: &mut SourceClosureState<'_>,
) -> Result<(), NativeBoundaryCompileError> {
    match context {
        CallableMaterializationContext::NoSubstitution => Ok(()),
        CallableMaterializationContext::Application(application) => {
            let path = WirePath::root().field(17);
            state.charge_relation(&path)?;
            collect_callable_application(
                application,
                exact_types,
                callable_applications,
                initialization_units,
                state,
                1,
            )?;
            Ok(())
        }
        CallableMaterializationContext::InitializationApplication(unit) => {
            let path = WirePath::root().field(21);
            state.charge_relation(&path)?;
            collect_initialization_application(unit, exact_types, initialization_units, state, 1)?;
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_callable_application(
    application: PersistentCallableApplicationId,
    exact_types: &HashMap<PersistentExactTypeId, ExactTypeKey>,
    callable_applications: &HashMap<PersistentCallableApplicationId, CallableApplicationKey>,
    initialization_units: &HashMap<PersistentInitializationUnitId, InitializationUnitKey>,
    state: &mut SourceClosureState<'_>,
    depth: u64,
) -> Result<u64, NativeBoundaryCompileError> {
    let path = WirePath::root().field(17);
    if let Some(height) = state.callable_heights.get(&application).copied() {
        state.check_subtree_depth(depth, height, &path)?;
        return Ok(height);
    }
    state.check_depth(depth, &path)?;
    let key = callable_applications
        .get(&application)
        .ok_or(NativeBoundaryCompileError::MissingCallableApplication { application })?;
    let mut height = 1;
    match key.instantiation_owner() {
        CallableInstantiationOwner::NoOwner => {}
        CallableInstantiationOwner::ExactNominalOwner(owner) => {
            state.charge_relation(&path)?;
            let child = collect_exact_type(owner, exact_types, state, depth + 1)?;
            height = height.max(child + 1);
        }
        CallableInstantiationOwner::EnclosingCallableApplication(enclosing) => {
            state.charge_relation(&path)?;
            let child = collect_callable_application(
                enclosing,
                exact_types,
                callable_applications,
                initialization_units,
                state,
                depth + 1,
            )?;
            height = height.max(child + 1);
        }
        CallableInstantiationOwner::EnclosingInitializationApplication(unit) => {
            state.charge_relation(&path)?;
            let child = collect_initialization_application(
                unit,
                exact_types,
                initialization_units,
                state,
                depth + 1,
            )?;
            height = height.max(child + 1);
        }
    }
    if let CallableArguments::Arguments(arguments) = key.callable_arguments() {
        for argument in arguments.as_slice() {
            state.charge_relation(&path)?;
            let child = collect_exact_type(*argument, exact_types, state, depth + 1)?;
            height = height.max(child + 1);
        }
    }
    state.cache_callable_height(application, height, &path)?;
    Ok(height)
}

fn collect_initialization_application(
    unit: PersistentInitializationUnitId,
    exact_types: &HashMap<PersistentExactTypeId, ExactTypeKey>,
    initialization_units: &HashMap<PersistentInitializationUnitId, InitializationUnitKey>,
    state: &mut SourceClosureState<'_>,
    depth: u64,
) -> Result<u64, NativeBoundaryCompileError> {
    let path = WirePath::root().field(21);
    if let Some(height) = state.initialization_heights.get(&unit).copied() {
        state.check_subtree_depth(depth, height, &path)?;
        return Ok(height);
    }
    state.check_depth(depth, &path)?;
    let key = initialization_units
        .get(&unit)
        .ok_or(NativeBoundaryCompileError::MissingInitializationUnit { unit })?;
    let mut height = 1;
    if let InitializationUnitKey::GenericDelegatedExtensionApplication {
        receiver_arguments, ..
    } = key
    {
        for argument in receiver_arguments.as_slice() {
            state.charge_relation(&path)?;
            let child = collect_exact_type(*argument, exact_types, state, depth + 1)?;
            height = height.max(child + 1);
        }
    }
    state.cache_initialization_height(unit, height, &path)?;
    Ok(height)
}

fn collect_exact_type(
    exact: PersistentExactTypeId,
    exact_types: &HashMap<PersistentExactTypeId, ExactTypeKey>,
    state: &mut SourceClosureState<'_>,
    depth: u64,
) -> Result<u64, NativeBoundaryCompileError> {
    let path = WirePath::root().field(15);
    if let Some(height) = state.exact_heights.get(&exact).copied() {
        state.check_subtree_depth(depth, height, &path)?;
        return Ok(height);
    }
    state.check_depth(depth, &path)?;
    let key = exact_types
        .get(&exact)
        .ok_or(NativeBoundaryCompileError::MissingExactType { exact })?;
    let mut height = 1;
    match key {
        ExactTypeKey::Nominal(owner) => {
            state.charge_relation(&path)?;
            state.require_owner(NativeBoundaryNominalOwner::Concrete(*owner), &path)?;
        }
        ExactTypeKey::NominalApplication { origin, arguments } => {
            state.charge_relation(&path)?;
            state.require_owner(NativeBoundaryNominalOwner::GenericTemplate(*origin), &path)?;
            for dependency in arguments.as_slice() {
                state.charge_relation(&path)?;
                let child = collect_exact_type(*dependency, exact_types, state, depth + 1)?;
                height = height.max(child + 1);
            }
        }
        ExactTypeKey::Tuple(elements) => {
            for dependency in elements.as_slice() {
                state.charge_relation(&path)?;
                let child = collect_exact_type(*dependency, exact_types, state, depth + 1)?;
                height = height.max(child + 1);
            }
        }
        ExactTypeKey::Function {
            parameters, result, ..
        }
        | ExactTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            for dependency in parameters {
                state.charge_relation(&path)?;
                let child = collect_exact_type(*dependency, exact_types, state, depth + 1)?;
                height = height.max(child + 1);
            }
            state.charge_relation(&path)?;
            let child = collect_exact_type(*result, exact_types, state, depth + 1)?;
            height = height.max(child + 1);
        }
        ExactTypeKey::RawPointer(pointee) => {
            state.charge_relation(&path)?;
            let child = collect_exact_type(*pointee, exact_types, state, depth + 1)?;
            height = height.max(child + 1);
        }
    }
    state.cache_exact_height(exact, height, &path)?;
    Ok(height)
}

#[derive(Debug)]
pub enum NativeBoundaryCompileError {
    Identity(IdentityValidationError),
    Resource(WireError),
    MissingCallableApplication {
        application: PersistentCallableApplicationId,
    },
    MissingInitializationUnit {
        unit: PersistentInitializationUnitId,
    },
    MissingExactType {
        exact: PersistentExactTypeId,
    },
    ClosureRequired {
        owner: NativeBoundaryNominalOwner,
    },
    UnrelatedDefinition {
        owner: NativeBoundaryNominalOwner,
    },
    Target(NativeBoundaryTargetError),
}

impl fmt::Display for NativeBoundaryCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
            Self::MissingCallableApplication { application } => write!(
                formatter,
                "native boundary references missing callable application {application}"
            ),
            Self::MissingInitializationUnit { unit } => write!(
                formatter,
                "native boundary references missing initialization application {unit}"
            ),
            Self::MissingExactType { exact } => {
                write!(
                    formatter,
                    "native boundary references missing exact type {exact}"
                )
            }
            Self::ClosureRequired { owner } => write!(
                formatter,
                "SLIB_CAPABILITY_NATIVE_BOUNDARY_CLOSURE_REQUIRED: no source witness for {owner:?}"
            ),
            Self::UnrelatedDefinition { owner } => write!(
                formatter,
                "native boundary contains unrelated source witness for {owner:?}"
            ),
            Self::Target(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeBoundaryCompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Target(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::CoreBuiltinNominal;
    use scoop_wire::{DecodeLimits, ResourceKind, WireErrorKind};

    use super::*;

    #[test]
    fn required_owner_queue_has_inclusive_heap_boundaries() {
        let ty = SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id());
        let path = WirePath::root().field(26);

        for (limit, accepted) in [(79, false), (80, true), (81, true)] {
            let mut meter = BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: limit,
                ..DecodeLimits::default()
            });
            let result = {
                let mut state = SourceClosureState::new(&mut meter);
                collect_signature_type(&ty, &mut state, &path, 1)
            };
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                assert_eq!(meter.usage().logical_heap_bytes, 80);
                assert_eq!(meter.usage().decoded_edges, 1);
            } else {
                assert!(matches!(
                    result,
                    Err(NativeBoundaryCompileError::Resource(ref error))
                        if error.kind() == &WireErrorKind::LimitExceeded {
                            resource: ResourceKind::LogicalHeapBytes,
                            limit: 79,
                            observed: 80,
                        }
                ));
            }
        }

        for (limit, accepted) in [(0, false), (1, true), (2, true)] {
            let mut meter = BudgetMeter::new(DecodeLimits {
                decoded_edges: limit,
                ..DecodeLimits::default()
            });
            let result = {
                let mut state = SourceClosureState::new(&mut meter);
                collect_signature_type(&ty, &mut state, &path, 1)
            };
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                assert_eq!(meter.usage().decoded_edges, 1);
            } else {
                assert!(matches!(
                    result,
                    Err(NativeBoundaryCompileError::Resource(ref error))
                        if error.kind() == &WireErrorKind::LimitExceeded {
                            resource: ResourceKind::DecodedEdges,
                            limit: 0,
                            observed: 1,
                        }
                ));
            }
        }
    }

    #[test]
    fn signature_walk_has_inclusive_semantic_depth_boundaries() {
        let ty = SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::RawPointer(Box::new(
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        ))));
        let path = WirePath::root().field(26);

        for (limit, accepted) in [(2, false), (3, true), (4, true)] {
            let mut meter = BudgetMeter::new(DecodeLimits {
                semantic_recursion: limit,
                ..DecodeLimits::default()
            });
            let result = {
                let mut state = SourceClosureState::new(&mut meter);
                collect_signature_type(&ty, &mut state, &path, 1)
            };
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert!(matches!(
                    result,
                    Err(NativeBoundaryCompileError::Resource(ref error))
                        if error.kind() == &WireErrorKind::LimitExceeded {
                            resource: ResourceKind::SemanticRecursion,
                            limit: 2,
                            observed: 3,
                        }
                ));
            }
        }
    }

    #[test]
    fn exact_type_height_cache_is_metered_and_rechecks_deeper_uses() {
        let unit = CoreBuiltinNominal::Unit.identity_record().id();
        let leaf = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit)).unwrap();
        let parent = CborIdentityRecord::from_key(ExactTypeKey::RawPointer(leaf.id())).unwrap();
        let leaf_id = leaf.id();
        let parent_id = parent.id();
        let exact_types =
            HashMap::from([(leaf_id, leaf.into_key()), (parent_id, parent.into_key())]);
        let mut meter = BudgetMeter::new(DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        });
        let error = {
            let mut state = SourceClosureState::new(&mut meter);
            assert_eq!(
                collect_exact_type(leaf_id, &exact_types, &mut state, 1).unwrap(),
                1
            );
            assert_eq!(state.exact_heights.len(), 1);
            collect_exact_type(parent_id, &exact_types, &mut state, 1).unwrap_err()
        };

        assert!(matches!(
            error,
            NativeBoundaryCompileError::Resource(ref error)
                if error.kind() == &WireErrorKind::LimitExceeded {
                    resource: ResourceKind::SemanticRecursion,
                    limit: 1,
                    observed: 2,
                }
        ));
    }

    #[test]
    fn exact_type_height_cache_avoids_duplicate_relation_charges() {
        let unit = CoreBuiltinNominal::Unit.identity_record().id();
        let record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit)).unwrap();
        let exact = record.id();
        let exact_types = HashMap::from([(exact, record.into_key())]);
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        {
            let mut state = SourceClosureState::new(&mut meter);
            assert_eq!(
                collect_exact_type(exact, &exact_types, &mut state, 1).unwrap(),
                1
            );
            assert_eq!(
                collect_exact_type(exact, &exact_types, &mut state, 2).unwrap(),
                1
            );
        }

        assert_eq!(meter.usage().decoded_edges, 1);
        assert_eq!(meter.usage().logical_heap_bytes, 112);
    }
}
