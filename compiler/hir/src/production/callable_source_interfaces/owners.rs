use scoop_identity::CallableTemplateOrigin;

use super::{
    CallableSourceInterfaceProductionError, CallableSourceProjection,
    SourceCallableOwnerProjectionError,
};
use crate::{ExportParameterOwner, HirClassConstructorIdentity, HirFunctionIdentity};

pub(super) struct SourceCallableOwner {
    pub(super) subject: crate::CallableProjectionSubject,
    pub(super) local: ExportParameterOwner,
    pub(super) declaration: CallableTemplateOrigin,
    pub(super) binders: Vec<crate::HirSignatureBinder>,
}

pub(super) fn collect_public(
    projection: &CallableSourceProjection<'_>,
) -> Result<Vec<SourceCallableOwner>, CallableSourceInterfaceProductionError> {
    let mut owners = Vec::new();
    for &function in &projection.export.public_surface.functions {
        let subject = crate::CallableProjectionSubject::Function(super::raw_index(function));
        owners.push(
            project_function(projection, function)
                .map_err(|error| production_error(subject, error))?,
        );
    }
    for &constructor in &projection.export.public_surface.struct_constructors {
        let subject =
            crate::CallableProjectionSubject::StructConstructor(super::raw_index(constructor));
        owners.push(
            project_struct_constructor(projection, constructor, subject)
                .map_err(|error| production_error(subject, error))?,
        );
    }
    for &constructor in &projection.export.public_surface.class_constructors {
        let subject =
            crate::CallableProjectionSubject::ClassConstructor(super::raw_index(constructor));
        if let Some(owner) = project_class_constructor(projection, constructor, subject)
            .map_err(|error| production_error(subject, error))?
        {
            owners.push(owner);
        }
    }
    collect_variants(projection, &mut owners)?;
    Ok(owners)
}

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

fn collect_variants(
    projection: &CallableSourceProjection<'_>,
    owners: &mut Vec<SourceCallableOwner>,
) -> Result<(), CallableSourceInterfaceProductionError> {
    for &enumeration_id in &projection.export.public_surface.enums {
        let enumeration =
            super::arena_get(&projection.export.enums, enumeration_id).ok_or_else(|| {
                production_error(
                    crate::CallableProjectionSubject::Variant {
                        enumeration: super::raw_index(enumeration_id),
                        variant: 0,
                    },
                    SourceCallableOwnerProjectionError::UnknownDeclaration,
                )
            })?;
        let binders = projection
            .signatures
            .binder_frame(&enumeration.type_params, 0)
            .map_err(|error| {
                production_error(
                    crate::CallableProjectionSubject::Variant {
                        enumeration: super::raw_index(enumeration_id),
                        variant: 0,
                    },
                    SourceCallableOwnerProjectionError::Signature(error),
                )
            })?;
        for variant_index in 0..enumeration.variants.len() {
            let variant_index = u32::try_from(variant_index).map_err(|_| {
                production_error(
                    crate::CallableProjectionSubject::Variant {
                        enumeration: super::raw_index(enumeration_id),
                        variant: u32::MAX,
                    },
                    SourceCallableOwnerProjectionError::TooManyVariants,
                )
            })?;
            let subject = crate::CallableProjectionSubject::Variant {
                enumeration: super::raw_index(enumeration_id),
                variant: variant_index,
            };
            let variant = crate::EnumVariantRef::checked(
                &projection.export.enums,
                enumeration_id,
                variant_index,
            )
            .ok_or_else(|| {
                production_error(
                    subject,
                    SourceCallableOwnerProjectionError::UnknownDeclaration,
                )
            })?;
            let identity = projection
                .export
                .enum_member_identities
                .get_variant(variant)
                .ok_or_else(|| {
                    production_error(subject, SourceCallableOwnerProjectionError::MissingIdentity)
                })?;
            let declaration = CallableTemplateOrigin::VariantConstructor(identity.id());
            require_callable(projection, declaration)
                .map_err(|error| production_error(subject, error))?;
            owners.push(SourceCallableOwner {
                subject,
                local: ExportParameterOwner::VariantConstructor(variant),
                declaration,
                binders: binders.clone(),
            });
        }
    }
    Ok(())
}

fn require_callable(
    projection: &CallableSourceProjection<'_>,
    declaration: CallableTemplateOrigin,
) -> Result<(), SourceCallableOwnerProjectionError> {
    if projection.callables.get(declaration).is_some() {
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
