use super::*;
use crate::CallableProjectionSubject as Subject;

pub(in super::super) fn collect(
    projection: &CallableSourceProjection<'_>,
) -> Result<Vec<SourceCallableOwner>, CallableSourceInterfaceProductionError> {
    let mut owners = Vec::new();
    for protocol in &projection.export.source_parameter_interfaces {
        let local = protocol.owner;
        let subject = subject(local);
        let declaration =
            declaration(projection, local).map_err(|error| production_error(subject, error))?;
        let Some(declaration) = declaration else {
            continue;
        };
        if projection.callables.declaration(declaration).is_none() {
            continue;
        }
        let projected = match local {
            ExportParameterOwner::Function(function) => {
                project_function(projection, function).map(Some)
            }
            ExportParameterOwner::StructConstructor(constructor) => {
                project_struct_constructor(projection, constructor, subject).map(Some)
            }
            ExportParameterOwner::ClassConstructor(constructor) => {
                project_class_constructor(projection, constructor, subject)
            }
            ExportParameterOwner::VariantConstructor(variant) => {
                project_variant(projection, variant, subject).map(Some)
            }
        }
        .map_err(|error| production_error(subject, error))?;
        let owner = projected.ok_or_else(|| {
            production_error(
                subject,
                SourceCallableOwnerProjectionError::UnknownDeclaration,
            )
        })?;
        owners.push(owner);
    }
    for callable in projection.callables.all_declarations() {
        let declaration = callable.declaration();
        if matches!(declaration, CallableTemplateOrigin::Accessor(_)) {
            continue;
        }
        if !owners.iter().any(|owner| owner.declaration == declaration) {
            return Err(CallableSourceInterfaceProductionError::MissingInterface(
                declaration,
            ));
        }
    }
    Ok(owners)
}

fn subject(owner: ExportParameterOwner) -> Subject {
    match owner {
        ExportParameterOwner::Function(id) => Subject::Function(super::super::raw_index(id)),
        ExportParameterOwner::StructConstructor(id) => {
            Subject::StructConstructor(super::super::raw_index(id))
        }
        ExportParameterOwner::ClassConstructor(id) => {
            Subject::ClassConstructor(super::super::raw_index(id))
        }
        ExportParameterOwner::VariantConstructor(variant) => Subject::Variant {
            enumeration: super::super::raw_index(variant.enumeration()),
            variant: variant.local_index(),
        },
    }
}

fn declaration(
    projection: &CallableSourceProjection<'_>,
    owner: ExportParameterOwner,
) -> Result<Option<CallableTemplateOrigin>, SourceCallableOwnerProjectionError> {
    use SourceCallableOwnerProjectionError as Error;
    let export = projection.export;
    Ok(match owner {
        ExportParameterOwner::Function(id) => match export
            .function_identities
            .get(id)
            .ok_or(Error::MissingIdentity)?
        {
            HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Plain(record)) => {
                Some(CallableTemplateOrigin::Function(record.id()))
            }
            HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Generic(record)) => {
                Some(CallableTemplateOrigin::GenericFunction(record.id()))
            }
            _ => None,
        },
        ExportParameterOwner::StructConstructor(id) => Some(CallableTemplateOrigin::Constructor(
            export
                .constructor_identities
                .get_struct(id)
                .ok_or(Error::MissingIdentity)?
                .id(),
        )),
        ExportParameterOwner::ClassConstructor(id) => export
            .constructor_identities
            .get_class(id)
            .ok_or(Error::MissingIdentity)?
            .source_record()
            .map(|record| CallableTemplateOrigin::Constructor(record.id())),
        ExportParameterOwner::VariantConstructor(variant) => {
            Some(CallableTemplateOrigin::VariantConstructor(
                export
                    .enum_member_identities
                    .get_variant(variant)
                    .ok_or(Error::MissingIdentity)?
                    .id(),
            ))
        }
    })
}
