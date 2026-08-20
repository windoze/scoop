use scoop_identity::CallableTemplateOrigin;

use super::{
    CallableSourceInterfaceProductionError, CallableSourceProjection, SourceCallableOwner,
    SourceCallableOwnerProjectionError,
};
use crate::{ExportParameterOwner, HirClassConstructorIdentity, HirFunctionIdentity};

mod selection;
pub(super) use selection::collect;

fn project_function(
    projection: &CallableSourceProjection<'_>,
    function_id: crate::FunctionId,
) -> Result<SourceCallableOwner, SourceCallableOwnerProjectionError> {
    let function = super::arena_get(&projection.export.functions, function_id)
        .ok_or(SourceCallableOwnerProjectionError::UnknownDeclaration)?;
    let identity = projection
        .export
        .function_identities
        .get(function_id)
        .ok_or(SourceCallableOwnerProjectionError::MissingIdentity)?;
    let HirFunctionIdentity::Source(identity) = identity else {
        return Err(SourceCallableOwnerProjectionError::NonSourceFunction);
    };
    let declaration = match identity {
        crate::HirSourceFunctionIdentity::Plain(record) => {
            CallableTemplateOrigin::Function(record.id())
        }
        crate::HirSourceFunctionIdentity::Generic(record) => {
            CallableTemplateOrigin::GenericFunction(record.id())
        }
    };
    let binders = projection
        .signatures
        .function_binders(function)
        .map_err(SourceCallableOwnerProjectionError::Signature)?;
    require_callable(projection, declaration)?;
    Ok(SourceCallableOwner {
        subject: crate::CallableProjectionSubject::Function(super::raw_index(function_id)),
        local: ExportParameterOwner::Function(function_id),
        declaration,
        binders,
    })
}

fn project_struct_constructor(
    projection: &CallableSourceProjection<'_>,
    constructor_id: crate::StructConstructorId,
    subject: crate::CallableProjectionSubject,
) -> Result<SourceCallableOwner, SourceCallableOwnerProjectionError> {
    let constructor = super::arena_get(&projection.export.struct_constructors, constructor_id)
        .ok_or(SourceCallableOwnerProjectionError::UnknownDeclaration)?;
    let owner = super::arena_get(&projection.export.structs, constructor.owner)
        .ok_or(SourceCallableOwnerProjectionError::UnknownNominalOwner)?;
    if !owner.constructors.contains(&constructor_id) {
        return Err(SourceCallableOwnerProjectionError::ConstructorOwnerMismatch);
    }
    let identity = projection
        .export
        .constructor_identities
        .get_struct(constructor_id)
        .ok_or(SourceCallableOwnerProjectionError::MissingIdentity)?;
    let declaration = CallableTemplateOrigin::Constructor(identity.id());
    let binders = projection
        .signatures
        .binder_frame(&owner.type_params, 0)
        .map_err(SourceCallableOwnerProjectionError::Signature)?;
    require_callable(projection, declaration)?;
    Ok(SourceCallableOwner {
        subject,
        local: ExportParameterOwner::StructConstructor(constructor_id),
        declaration,
        binders,
    })
}

fn project_class_constructor(
    projection: &CallableSourceProjection<'_>,
    constructor_id: crate::ClassConstructorId,
    subject: crate::CallableProjectionSubject,
) -> Result<Option<SourceCallableOwner>, SourceCallableOwnerProjectionError> {
    let constructor = super::arena_get(&projection.export.class_constructors, constructor_id)
        .ok_or(SourceCallableOwnerProjectionError::UnknownDeclaration)?;
    let owner = super::arena_get(&projection.export.classes, constructor.owner)
        .ok_or(SourceCallableOwnerProjectionError::UnknownNominalOwner)?;
    if !owner.constructors.contains(&constructor_id) {
        return Err(SourceCallableOwnerProjectionError::ConstructorOwnerMismatch);
    }
    let identity = projection
        .export
        .constructor_identities
        .get_class(constructor_id)
        .ok_or(SourceCallableOwnerProjectionError::MissingIdentity)?;
    let HirClassConstructorIdentity::Source(identity) = identity else {
        return Ok(None);
    };
    let declaration = CallableTemplateOrigin::Constructor(identity.id());
    let binders = projection
        .signatures
        .binder_frame(&owner.type_params, 0)
        .map_err(SourceCallableOwnerProjectionError::Signature)?;
    require_callable(projection, declaration)?;
    Ok(Some(SourceCallableOwner {
        subject,
        local: ExportParameterOwner::ClassConstructor(constructor_id),
        declaration,
        binders,
    }))
}

fn project_variant(
    projection: &CallableSourceProjection<'_>,
    variant: crate::EnumVariantRef,
    subject: crate::CallableProjectionSubject,
) -> Result<SourceCallableOwner, SourceCallableOwnerProjectionError> {
    let enumeration = super::arena_get(&projection.export.enums, variant.enumeration())
        .ok_or(SourceCallableOwnerProjectionError::UnknownNominalOwner)?;
    let identity = projection
        .export
        .enum_member_identities
        .get_variant(variant)
        .ok_or(SourceCallableOwnerProjectionError::MissingIdentity)?;
    let declaration = CallableTemplateOrigin::VariantConstructor(identity.id());
    require_callable(projection, declaration)?;
    let binders = projection
        .signatures
        .binder_frame(&enumeration.type_params, 0)
        .map_err(SourceCallableOwnerProjectionError::Signature)?;
    Ok(SourceCallableOwner {
        subject,
        local: ExportParameterOwner::VariantConstructor(variant),
        declaration,
        binders,
    })
}

fn require_callable(
    projection: &CallableSourceProjection<'_>,
    declaration: CallableTemplateOrigin,
) -> Result<(), SourceCallableOwnerProjectionError> {
    if projection.callables.declaration(declaration).is_some() {
        Ok(())
    } else {
        Err(SourceCallableOwnerProjectionError::MissingCallableInterface(declaration))
    }
}

fn production_error(
    subject: crate::CallableProjectionSubject,
    error: SourceCallableOwnerProjectionError,
) -> CallableSourceInterfaceProductionError {
    CallableSourceInterfaceProductionError::Owner { subject, error }
}
