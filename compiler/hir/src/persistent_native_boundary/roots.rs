use std::collections::BTreeSet;

use scoop_identity::{
    CallableArguments, CallableInstantiationOwner, CallableMaterializationContext, ExactTypeKey,
    InitializationUnitKey, OptionalSignatureType, PersistentCallableApplicationId,
    PersistentExactTypeId, PersistentInitializationUnitId, SignatureCallableShape,
    SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceExternFunctionAbi,
    SourceNativeExternalContract, SourceScoopAbiFunctionSignature,
};

use super::{HirNativeBoundaryTypeDefinitionError, HirNativeBoundaryTypeDefinitionInputs};
use crate::NativeBoundaryNominalOwner;

pub(super) fn collect(
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
) -> Result<BTreeSet<NativeBoundaryNominalOwner>, HirNativeBoundaryTypeDefinitionError> {
    let mut required = BTreeSet::new();
    collect_contracts(inputs, &mut required);
    collect_callbacks(inputs, &mut required)?;
    Ok(required)
}

fn collect_contracts(
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) {
    for entry in inputs.source_native_contracts.iter() {
        match entry.record().contract() {
            SourceNativeExternalContract::Function { abi, .. } => match abi {
                SourceExternFunctionAbi::C(signature) => collect_c_signature(signature, required),
                SourceExternFunctionAbi::Scoop { signature, .. } => {
                    collect_scoop_signature(signature, required)
                }
            },
            SourceNativeExternalContract::ReadOnlyData { storage, .. }
            | SourceNativeExternalContract::MutableData { storage, .. }
            | SourceNativeExternalContract::ReadOnlyTls { storage, .. }
            | SourceNativeExternalContract::MutableTls { storage, .. } => {
                collect_signature_type(storage, required)
            }
        }
    }
}

fn collect_callbacks(
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) -> Result<(), HirNativeBoundaryTypeDefinitionError> {
    for record in inputs.callback_registration_identities.records() {
        collect_c_signature(record.key().source_signature(), required);
        collect_callable_shape(record.key().managed_signature(), required);
    }

    let mut visited_applications = BTreeSet::new();
    let mut visited_exact_types = BTreeSet::new();
    for application in inputs.local.callback_applications.records() {
        collect_materialization_context(
            application.key().context(),
            inputs,
            required,
            &mut visited_applications,
            &mut visited_exact_types,
        )?;
    }
    Ok(())
}

fn collect_materialization_context(
    context: CallableMaterializationContext,
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
    visited_applications: &mut BTreeSet<PersistentCallableApplicationId>,
    visited_exact_types: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), HirNativeBoundaryTypeDefinitionError> {
    match context {
        CallableMaterializationContext::NoSubstitution => Ok(()),
        CallableMaterializationContext::Application(application) => collect_callable_application(
            application,
            inputs,
            required,
            visited_applications,
            visited_exact_types,
        ),
        CallableMaterializationContext::InitializationApplication(unit) => {
            collect_initialization_application(unit, inputs, required, visited_exact_types)
        }
    }
}

fn collect_callable_application(
    application: PersistentCallableApplicationId,
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
    visited_applications: &mut BTreeSet<PersistentCallableApplicationId>,
    visited_exact_types: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), HirNativeBoundaryTypeDefinitionError> {
    if !visited_applications.insert(application) {
        return Ok(());
    }
    let record =
        inputs.local.callable_applications.get(application).ok_or(
            HirNativeBoundaryTypeDefinitionError::MissingCallableApplication { application },
        )?;
    match record.key().instantiation_owner() {
        CallableInstantiationOwner::NoOwner => {}
        CallableInstantiationOwner::ExactNominalOwner(owner) => {
            collect_exact_type(owner, inputs, required, visited_exact_types)?
        }
        CallableInstantiationOwner::EnclosingCallableApplication(enclosing) => {
            collect_callable_application(
                enclosing,
                inputs,
                required,
                visited_applications,
                visited_exact_types,
            )?;
        }
        CallableInstantiationOwner::EnclosingInitializationApplication(unit) => {
            collect_initialization_application(unit, inputs, required, visited_exact_types)?;
        }
    }
    if let CallableArguments::Arguments(arguments) = record.key().callable_arguments() {
        for argument in arguments.as_slice() {
            collect_exact_type(*argument, inputs, required, visited_exact_types)?;
        }
    }
    Ok(())
}

fn collect_initialization_application(
    unit: PersistentInitializationUnitId,
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
    visited_exact_types: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), HirNativeBoundaryTypeDefinitionError> {
    let record = inputs
        .initialization_unit_identities
        .records()
        .iter()
        .find(|record| record.id() == unit)
        .ok_or(HirNativeBoundaryTypeDefinitionError::MissingInitializationApplication { unit })?;
    if let InitializationUnitKey::GenericDelegatedExtensionApplication {
        receiver_arguments, ..
    } = record.key()
    {
        for argument in receiver_arguments.as_slice() {
            collect_exact_type(*argument, inputs, required, visited_exact_types)?;
        }
    }
    Ok(())
}

fn collect_exact_type(
    exact: PersistentExactTypeId,
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
    visited: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), HirNativeBoundaryTypeDefinitionError> {
    if !visited.insert(exact) {
        return Ok(());
    }
    let ty = inputs
        .local
        .exact_type_identities
        .type_for_identity(exact)
        .ok_or(HirNativeBoundaryTypeDefinitionError::MissingExactType { exact })?;
    let record = &inputs.local.exact_type_identities[ty];
    match record.key() {
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
    for dependency in record.key().exact_type_dependencies() {
        collect_exact_type(dependency, inputs, required, visited)?;
    }
    Ok(())
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

fn collect_callable_shape(
    shape: &SignatureCallableShape,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) {
    if let OptionalSignatureType::Present(receiver) = shape.receiver() {
        collect_signature_type(receiver, required);
    }
    for parameter in shape.parameters() {
        collect_signature_type(parameter, required);
    }
    collect_signature_type(shape.result(), required);
}

pub(super) fn collect_signature_type(
    ty: &SignatureTypeKey,
    required: &mut BTreeSet<NativeBoundaryNominalOwner>,
) {
    match ty {
        SignatureTypeKey::Nominal(id) => {
            required.insert(NativeBoundaryNominalOwner::Concrete(*id));
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
