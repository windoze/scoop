use scoop_identity::{
    CallableTemplateOrigin, DefinitionOwnerAtom, DuplicateSignatureKey, NominalDeclarationOwner,
    OptionalSignatureType, SourceDeclarationKind,
};

use super::{
    CallableInterfaceBuildError, CallableProjection, CallableProjectionError,
    CallableProjectionSubject, access, effects, parameters,
};
use crate::{
    CallableDeclarationRecordV1, CallableInterfaceRecordV1, CallableModalityV1, Function,
    FunctionGenericity, HirFunctionIdentity, HirSourceFunctionIdentity,
    InterfaceMemberImplementation, InterfaceMemberRole, MethodDispatch, MethodModifier,
    PublicDeclarationOwnerV1, TypeParamDecl,
};

pub(super) fn project_all(
    projection: &CallableProjection<'_>,
    records: &mut Vec<CallableInterfaceRecordV1>,
) -> Result<(), CallableInterfaceBuildError> {
    for &function_id in &projection.export.public_surface.functions {
        let subject = CallableProjectionSubject::Function(super::raw_index(function_id));
        let record = project(projection, function_id)
            .and_then(|data| {
                let access = access::project(&projection.export.functions[function_id].access)
                    .map_err(CallableProjectionError::Access)?;
                CallableInterfaceRecordV1::from_declaration(data, access)
                    .map_err(CallableProjectionError::Record)
            })
            .map_err(|error| CallableInterfaceBuildError::projection(subject, error))?;
        records.push(record);
    }
    Ok(())
}

pub(super) fn project(
    projection: &CallableProjection<'_>,
    function_id: crate::FunctionId,
) -> Result<CallableDeclarationRecordV1, CallableProjectionError> {
    let function = super::arena_get(&projection.export.functions, function_id)
        .ok_or(CallableProjectionError::UnknownDeclaration)?;
    let identity = projection
        .export
        .function_identities
        .get(function_id)
        .ok_or(CallableProjectionError::MissingIdentity)?;
    let HirFunctionIdentity::Source(source) = identity else {
        return Err(CallableProjectionError::NonSourceIdentity);
    };
    let declaration = callable_declaration(source);
    let key = source.declaration();
    validate_source_key(projection, key)?;

    let (own_parameters, _) = parameter_groups(function);
    let own_arity = u32::try_from(own_parameters.len()).map_err(|_| {
        CallableProjectionError::TypeParameterArity {
            expected: key.duplicate_signature().type_parameter_count(),
            actual: u32::MAX,
        }
    })?;
    if key.duplicate_signature().type_parameter_count() != own_arity {
        return Err(CallableProjectionError::TypeParameterArity {
            expected: key.duplicate_signature().type_parameter_count(),
            actual: own_arity,
        });
    }
    validate_identity_kind(source, own_arity)?;

    let DuplicateSignatureKey::Function {
        receiver,
        parameters: identity_parameters,
        ..
    } = key.duplicate_signature()
    else {
        return Err(CallableProjectionError::InvalidDeclarationKind {
            expected: SourceDeclarationKind::Function,
            actual: key.declaration_kind(),
        });
    };
    let receiver = optional_signature(receiver);
    let owner = project_owner(projection, function_id, function, receiver.is_some(), key)?;
    let binders = projection
        .signatures
        .function_binders(function)
        .map_err(CallableProjectionError::Signature)?;
    let type_parameters = projection
        .signatures
        .project_binder_list(&own_parameters, &binders)
        .map_err(CallableProjectionError::Signature)?;
    let parameters = parameters::project(
        projection,
        crate::ExportParameterOwner::Function(function_id),
        &binders,
        identity_parameters,
    )
    .map_err(CallableProjectionError::Parameters)?;
    let result = projection
        .signatures
        .map_type(function.return_ty, &binders)
        .map_err(CallableProjectionError::Signature)?;
    let effects = effects::function(projection.export, function, &binders)
        .map_err(CallableProjectionError::Effects)?;
    let modality = modality(projection, function_id, function)?;

    CallableDeclarationRecordV1::try_new(
        declaration,
        owner,
        type_parameters,
        receiver,
        parameters,
        result,
        effects,
        modality,
        function.access.declared.into(),
        super::slots::method(projection.export, function.method)?,
        parameters::context(&projection.signatures, function, &binders)
            .map_err(CallableProjectionError::Parameters)?,
    )
    .map_err(CallableProjectionError::Record)
}

fn validate_source_key(
    projection: &CallableProjection<'_>,
    key: &scoop_identity::SourceDeclarationKey,
) -> Result<(), CallableProjectionError> {
    if key.origin() != projection.export.cone {
        return Err(CallableProjectionError::ForeignDeclaration {
            expected: projection.export.cone,
            actual: key.origin(),
        });
    }
    if key.declaration_kind() != SourceDeclarationKind::Function {
        return Err(CallableProjectionError::InvalidDeclarationKind {
            expected: SourceDeclarationKind::Function,
            actual: key.declaration_kind(),
        });
    }
    Ok(())
}

fn validate_identity_kind(
    identity: &HirSourceFunctionIdentity,
    own_arity: u32,
) -> Result<(), CallableProjectionError> {
    if matches!(
        (identity, own_arity),
        (HirSourceFunctionIdentity::Plain(_), 0)
    ) || matches!(identity, HirSourceFunctionIdentity::Generic(_)) && own_arity != 0
    {
        Ok(())
    } else {
        Err(CallableProjectionError::InvalidIdentityKind)
    }
}

fn callable_declaration(identity: &HirSourceFunctionIdentity) -> CallableTemplateOrigin {
    match identity {
        HirSourceFunctionIdentity::Plain(record) => CallableTemplateOrigin::Function(record.id()),
        HirSourceFunctionIdentity::Generic(record) => {
            CallableTemplateOrigin::GenericFunction(record.id())
        }
    }
}

fn parameter_groups(function: &Function) -> (Vec<TypeParamDecl>, Vec<TypeParamDecl>) {
    match &function.genericity {
        FunctionGenericity::Plain => (Vec::new(), Vec::new()),
        FunctionGenericity::Generic { parameters, .. } => (parameters.clone(), Vec::new()),
        FunctionGenericity::OwnerParameterizedMethod {
            owner_parameters, ..
        } => (Vec::new(), owner_parameters.clone()),
        FunctionGenericity::GenericMethod {
            owner_parameters,
            method_parameters,
            ..
        } => (
            method_parameters.iter().cloned().collect(),
            owner_parameters.clone(),
        ),
    }
}

fn project_owner(
    projection: &CallableProjection<'_>,
    _function_id: crate::FunctionId,
    function: &Function,
    has_receiver: bool,
    key: &scoop_identity::SourceDeclarationKey,
) -> Result<PublicDeclarationOwnerV1, CallableProjectionError> {
    let owner = if let Some(method) = function.method {
        if has_receiver {
            return Err(CallableProjectionError::InvalidOwner);
        }
        let owner = super::super::nominal_interfaces::owner_resolution::from_type(
            projection.export,
            method.owner,
        )
        .ok_or(CallableProjectionError::MissingNominalOwner)?;
        let expected = owner_atom(owner);
        if key.owners().owners().last() != Some(&expected) {
            return Err(CallableProjectionError::PersistentOwnerMismatch);
        }
        PublicDeclarationOwnerV1::Nominal(owner)
    } else {
        if !key.owners().owners().is_empty() {
            return Err(CallableProjectionError::PersistentOwnerMismatch);
        }
        if has_receiver {
            PublicDeclarationOwnerV1::Extension
        } else {
            PublicDeclarationOwnerV1::TopLevel
        }
    };
    Ok(owner)
}

fn modality(
    projection: &CallableProjection<'_>,
    function_id: crate::FunctionId,
    function: &Function,
) -> Result<CallableModalityV1, CallableProjectionError> {
    let Some(method) = function.method else {
        return Ok(CallableModalityV1::Final);
    };
    if let MethodDispatch::Interface(member_id) = method.dispatch {
        let member = super::arena_get(&projection.export.interface_methods, member_id).ok_or(
            CallableProjectionError::MissingInterfaceMethod(super::raw_index(member_id)),
        )?;
        if member.function != function_id || member.role != InterfaceMemberRole::Function {
            return Err(CallableProjectionError::InterfaceMethodMismatch);
        }
        return Ok(match member.implementation {
            InterfaceMemberImplementation::Body => CallableModalityV1::InterfaceDefault,
            InterfaceMemberImplementation::AbstractSlot => CallableModalityV1::Abstract,
        });
    }
    Ok(match method.modifier {
        MethodModifier::Final => CallableModalityV1::Final,
        MethodModifier::Open => CallableModalityV1::Open,
        MethodModifier::Abstract => CallableModalityV1::Abstract,
    })
}

fn optional_signature(value: &OptionalSignatureType) -> Option<scoop_identity::SignatureTypeKey> {
    match value {
        OptionalSignatureType::Absent => None,
        OptionalSignatureType::Present(value) => Some(value.as_ref().clone()),
    }
}

fn owner_atom(owner: NominalDeclarationOwner) -> DefinitionOwnerAtom {
    match owner {
        NominalDeclarationOwner::Concrete(id) => DefinitionOwnerAtom::Type(id),
        NominalDeclarationOwner::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(id),
    }
}
