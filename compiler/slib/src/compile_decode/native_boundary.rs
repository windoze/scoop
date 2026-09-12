use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

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
use scoop_wire::WirePath;

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
    let exact_types = records_by_id([
        graph.records::<PersistentExactTypeId, ExactTypeKey>(
            IdentityLayer::Hir,
            meter,
            &WirePath::root().field(15),
        ),
        graph.records::<PersistentExactTypeId, ExactTypeKey>(
            IdentityLayer::Mir,
            meter,
            &WirePath::root().field(1),
        ),
        graph.records::<PersistentExactTypeId, ExactTypeKey>(
            IdentityLayer::Lir,
            meter,
            &WirePath::root().field(1),
        ),
    ])?;
    let callable_applications = records_by_id(std::iter::once(
        graph.records::<PersistentCallableApplicationId, CallableApplicationKey>(
            IdentityLayer::Hir,
            meter,
            &WirePath::root().field(17),
        ),
    ))?;
    let initialization_units = records_by_id(std::iter::once(
        graph.records::<PersistentInitializationUnitId, InitializationUnitKey>(
            IdentityLayer::Hir,
            meter,
            &WirePath::root().field(21),
        ),
    ))?;
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

    let mut required = BTreeSet::new();
    for source in foundations.hir.source_native_contracts() {
        collect_source_contract(source.contract(), &mut required);
    }
    for registration in callback_registrations {
        collect_c_signature(registration.key().source_signature(), &mut required);
        collect_optional_signature_type(
            registration.key().managed_signature().receiver(),
            &mut required,
        );
        for parameter in registration.key().managed_signature().parameters() {
            collect_signature_type(parameter, &mut required);
        }
        collect_signature_type(
            registration.key().managed_signature().result(),
            &mut required,
        );
    }

    let mut visited_callable_applications = BTreeSet::new();
    let mut visited_exact_types = BTreeSet::new();
    for application in callback_applications {
        collect_materialization_context(
            application.key().context(),
            &exact_types,
            &callable_applications,
            &initialization_units,
            &mut required,
            &mut visited_callable_applications,
            &mut visited_exact_types,
        )?;
    }

    let definitions = foundations
        .hir
        .native_boundary_types()
        .iter()
        .map(|definition| (definition.owner(), definition))
        .collect::<BTreeMap<_, _>>();
    let mut expanded = BTreeSet::new();
    while let Some(owner) = required.pop_first() {
        if !expanded.insert(owner) {
            continue;
        }
        let definition = definitions
            .get(&owner)
            .ok_or(NativeBoundaryCompileError::ClosureRequired { owner })?;
        match definition.shape() {
            scoop_hir::NativeBoundaryNominalShape::Reference => {}
            scoop_hir::NativeBoundaryNominalShape::Struct { fields, .. } => {
                for field in fields {
                    collect_signature_type(field.ty(), &mut required);
                }
            }
            scoop_hir::NativeBoundaryNominalShape::Enum { variants } => {
                for variant in variants {
                    for field in variant.fields() {
                        collect_signature_type(field.ty(), &mut required);
                    }
                }
            }
        }
    }
    if let Some(owner) = definitions
        .keys()
        .copied()
        .find(|owner| !expanded.contains(owner))
    {
        return Err(NativeBoundaryCompileError::UnrelatedDefinition { owner });
    }
    Ok(())
}

fn records_by_id<I, K>(
    results: impl IntoIterator<Item = Result<Vec<CborIdentityRecord<I, K>>, IdentityValidationError>>,
) -> Result<BTreeMap<I, K>, NativeBoundaryCompileError>
where
    I: scoop_identity::PersistentId + Ord,
    K: Clone,
{
    let mut records = BTreeMap::new();
    for result in results {
        for record in result.map_err(NativeBoundaryCompileError::Identity)? {
            records.insert(record.id(), record.into_key());
        }
    }
    Ok(records)
}

fn collect_source_contract(
    contract: &SourceNativeExternalContract,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) {
    match contract {
        SourceNativeExternalContract::Function { abi, .. } => match abi {
            SourceExternFunctionAbi::C(signature) => collect_c_signature(signature, required),
            SourceExternFunctionAbi::Scoop { signature, .. } => {
                collect_scoop_signature(signature, required);
            }
        },
        SourceNativeExternalContract::ReadOnlyData { storage, .. }
        | SourceNativeExternalContract::MutableData { storage, .. }
        | SourceNativeExternalContract::ReadOnlyTls { storage, .. }
        | SourceNativeExternalContract::MutableTls { storage, .. } => {
            collect_signature_type(storage, required);
        }
    }
}

fn collect_c_signature(
    signature: &SourceCAbiFunctionSignature,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) {
    for parameter in signature.parameters() {
        collect_signature_type(parameter, required);
    }
    if let SourceCAbiReturn::Value(result) = signature.result() {
        collect_signature_type(result, required);
    }
}

fn collect_scoop_signature(
    signature: &SourceScoopAbiFunctionSignature,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) {
    for parameter in signature.parameters() {
        collect_signature_type(parameter, required);
    }
    collect_signature_type(signature.result(), required);
}

fn collect_optional_signature_type(
    ty: &OptionalSignatureType,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) {
    if let OptionalSignatureType::Present(ty) = ty {
        collect_signature_type(ty, required);
    }
}

fn collect_signature_type(
    ty: &SignatureTypeKey,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) {
    match ty {
        SignatureTypeKey::Nominal(owner) => {
            required.insert(NativeBoundaryNominalOwner::Concrete(*owner));
        }
        SignatureTypeKey::NominalApplication { origin, arguments } => {
            required.insert(NativeBoundaryNominalOwner::GenericTemplate(*origin));
            for argument in arguments.as_slice() {
                collect_signature_type(argument, required);
            }
        }
        SignatureTypeKey::Tuple(elements) => {
            for element in elements.as_slice() {
                collect_signature_type(element, required);
            }
        }
        SignatureTypeKey::Function {
            parameters, result, ..
        }
        | SignatureTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            for parameter in parameters {
                collect_signature_type(parameter, required);
            }
            collect_signature_type(result, required);
        }
        SignatureTypeKey::RawPointer(pointee) => collect_signature_type(pointee, required),
        SignatureTypeKey::Binder { .. } => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_materialization_context(
    context: CallableMaterializationContext,
    exact_types: &BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    callable_applications: &BTreeMap<PersistentCallableApplicationId, CallableApplicationKey>,
    initialization_units: &BTreeMap<PersistentInitializationUnitId, InitializationUnitKey>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
    visited_callable_applications: &mut BTreeSet<PersistentCallableApplicationId>,
    visited_exact_types: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), NativeBoundaryCompileError> {
    match context {
        CallableMaterializationContext::NoSubstitution => Ok(()),
        CallableMaterializationContext::Application(application) => collect_callable_application(
            application,
            exact_types,
            callable_applications,
            initialization_units,
            required,
            visited_callable_applications,
            visited_exact_types,
        ),
        CallableMaterializationContext::InitializationApplication(unit) => {
            collect_initialization_application(
                unit,
                exact_types,
                initialization_units,
                required,
                visited_exact_types,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_callable_application(
    application: PersistentCallableApplicationId,
    exact_types: &BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    callable_applications: &BTreeMap<PersistentCallableApplicationId, CallableApplicationKey>,
    initialization_units: &BTreeMap<PersistentInitializationUnitId, InitializationUnitKey>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
    visited_callable_applications: &mut BTreeSet<PersistentCallableApplicationId>,
    visited_exact_types: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), NativeBoundaryCompileError> {
    if !visited_callable_applications.insert(application) {
        return Ok(());
    }
    let key = callable_applications
        .get(&application)
        .ok_or(NativeBoundaryCompileError::MissingCallableApplication { application })?;
    match key.instantiation_owner() {
        CallableInstantiationOwner::NoOwner => {}
        CallableInstantiationOwner::ExactNominalOwner(owner) => {
            collect_exact_type(owner, exact_types, required, visited_exact_types)?
        }
        CallableInstantiationOwner::EnclosingCallableApplication(enclosing) => {
            collect_callable_application(
                enclosing,
                exact_types,
                callable_applications,
                initialization_units,
                required,
                visited_callable_applications,
                visited_exact_types,
            )?;
        }
        CallableInstantiationOwner::EnclosingInitializationApplication(unit) => {
            collect_initialization_application(
                unit,
                exact_types,
                initialization_units,
                required,
                visited_exact_types,
            )?;
        }
    }
    if let CallableArguments::Arguments(arguments) = key.callable_arguments() {
        for argument in arguments.as_slice() {
            collect_exact_type(*argument, exact_types, required, visited_exact_types)?;
        }
    }
    Ok(())
}

fn collect_initialization_application(
    unit: PersistentInitializationUnitId,
    exact_types: &BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    initialization_units: &BTreeMap<PersistentInitializationUnitId, InitializationUnitKey>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
    visited_exact_types: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), NativeBoundaryCompileError> {
    let key = initialization_units
        .get(&unit)
        .ok_or(NativeBoundaryCompileError::MissingInitializationUnit { unit })?;
    if let InitializationUnitKey::GenericDelegatedExtensionApplication {
        receiver_arguments, ..
    } = key
    {
        for argument in receiver_arguments.as_slice() {
            collect_exact_type(*argument, exact_types, required, visited_exact_types)?;
        }
    }
    Ok(())
}

fn collect_exact_type(
    exact: PersistentExactTypeId,
    exact_types: &BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
    visited: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), NativeBoundaryCompileError> {
    if !visited.insert(exact) {
        return Ok(());
    }
    let key = exact_types
        .get(&exact)
        .ok_or(NativeBoundaryCompileError::MissingExactType { exact })?;
    match key {
        ExactTypeKey::Nominal(owner) => {
            required.insert(NativeBoundaryNominalOwner::Concrete(*owner));
        }
        ExactTypeKey::NominalApplication { origin, .. } => {
            required.insert(NativeBoundaryNominalOwner::GenericTemplate(*origin));
        }
        ExactTypeKey::Tuple(_)
        | ExactTypeKey::Function { .. }
        | ExactTypeKey::RawPointer(_)
        | ExactTypeKey::NativeFunctionPointer { .. } => {}
    }
    for dependency in key.exact_type_dependencies() {
        collect_exact_type(dependency, exact_types, required, visited)?;
    }
    Ok(())
}

#[derive(Debug)]
pub enum NativeBoundaryCompileError {
    Identity(IdentityValidationError),
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

impl std::error::Error for NativeBoundaryCompileError {}
