use scoop_identity::{CanonicalIdentifier, SignatureTypeKey};

use super::{
    CallableSourceInterfaceProductionError, CallableSourceParameterProjectionError,
    CallableSourceProjection, SourceCallableOwner,
};
use crate::{
    CallableParameterCallingV1, CallableSourceInterfaceV1, CallableSourceParameterV1,
    CanonicalCallableSourceParametersV1, ExportDefaultTemplateKeyV1, ExportParameterCalling,
    ExportParameterInterface, ExportVarargOmission, HirSignatureBinder, TypeId,
};

pub(super) fn project(
    projection: &CallableSourceProjection<'_>,
    owner: SourceCallableOwner,
) -> Result<CallableSourceInterfaceV1, CallableSourceInterfaceProductionError> {
    let result = project_parameters(projection, &owner);
    result.map_err(|error| CallableSourceInterfaceProductionError::Parameters {
        subject: owner.subject,
        error,
    })
}

fn project_parameters(
    projection: &CallableSourceProjection<'_>,
    owner: &SourceCallableOwner,
) -> Result<CallableSourceInterfaceV1, CallableSourceParameterProjectionError> {
    let source = unique_interface(projection.export, owner.local)?;
    let callable = projection.callables.get(owner.declaration).ok_or(
        CallableSourceParameterProjectionError::MissingCallableInterface(owner.declaration),
    )?;
    let expected = callable.parameters().parameters();
    if source.parameters.len() != expected.len() {
        return Err(CallableSourceParameterProjectionError::Arity {
            expected: expected.len(),
            actual: source.parameters.len(),
        });
    }

    let mut parameters = Vec::with_capacity(source.parameters.len());
    for (index, (parameter, expected)) in source.parameters.iter().zip(expected).enumerate() {
        let position = u32::try_from(index)
            .map_err(|_| CallableSourceParameterProjectionError::TooManyParameters)?;
        let name = CanonicalIdentifier::new(&parameter.name).map_err(|source| {
            CallableSourceParameterProjectionError::InvalidName { position, source }
        })?;
        if &name != expected.name() {
            return Err(CallableSourceParameterProjectionError::NameMismatch {
                position,
                expected: expected.name().clone(),
                actual: name,
            });
        }
        let (value_type, calling) = project_calling(
            projection,
            owner.declaration,
            position,
            parameter.calling,
            &owner.binders,
        )?;
        if &value_type != expected.value_type() {
            return Err(CallableSourceParameterProjectionError::TypeMismatch {
                position,
                expected: Box::new(expected.value_type().clone()),
                actual: Box::new(value_type),
            });
        }
        let definition_origin = super::super::definition_sources::project_definition_source(
            projection.export,
            parameter.origin,
        )
        .map_err(
            |source| CallableSourceParameterProjectionError::DefinitionOrigin { position, source },
        )?;
        parameters.push(CallableSourceParameterV1::new(
            name,
            expected.value_type().clone(),
            calling,
            definition_origin,
        ));
    }
    let parameters = CanonicalCallableSourceParametersV1::try_new(parameters)
        .map_err(CallableSourceParameterProjectionError::ParameterList)?;
    CallableSourceInterfaceV1::try_new(owner.declaration, parameters)
        .map_err(CallableSourceParameterProjectionError::Record)
}

fn project_calling(
    projection: &CallableSourceProjection<'_>,
    owner: scoop_identity::CallableTemplateOrigin,
    position: u32,
    calling: ExportParameterCalling,
    binders: &[HirSignatureBinder],
) -> Result<(SignatureTypeKey, CallableParameterCallingV1), CallableSourceParameterProjectionError>
{
    let key = ExportDefaultTemplateKeyV1::new(owner, position);
    match calling {
        ExportParameterCalling::Required { value_type } => Ok((
            map_type(projection, position, value_type, binders)?,
            CallableParameterCallingV1::Required,
        )),
        ExportParameterCalling::Default { value_type, source } => {
            require_default_source(projection, position, source)?;
            Ok((
                map_type(projection, position, value_type, binders)?,
                CallableParameterCallingV1::Default { template: key },
            ))
        }
        ExportParameterCalling::Vararg {
            parameter_type,
            omission,
        } => {
            let parameter = super::arena_get(
                &projection.export.export_vararg_parameter_types,
                parameter_type,
            )
            .ok_or(CallableSourceParameterProjectionError::UnknownVarargType {
                position,
                parameter_type: super::raw_index(parameter_type),
            })?;
            let value_type = map_type(projection, position, parameter.array_type, binders)?;
            let element_type = map_type(projection, position, parameter.element_type, binders)?;
            let calling = match omission {
                ExportVarargOmission::EmptyArray => {
                    CallableParameterCallingV1::VarargEmpty { element_type }
                }
                ExportVarargOmission::Default(source) => {
                    require_default_source(projection, position, source)?;
                    CallableParameterCallingV1::VarargDefault {
                        element_type,
                        template: key,
                    }
                }
            };
            Ok((value_type, calling))
        }
    }
}

fn map_type(
    projection: &CallableSourceProjection<'_>,
    position: u32,
    ty: TypeId,
    binders: &[HirSignatureBinder],
) -> Result<SignatureTypeKey, CallableSourceParameterProjectionError> {
    projection
        .signatures
        .map_type(ty, binders)
        .map_err(|source| CallableSourceParameterProjectionError::Signature { position, source })
}

fn require_default_source(
    projection: &CallableSourceProjection<'_>,
    position: u32,
    source: crate::ExportDefaultSourceId,
) -> Result<(), CallableSourceParameterProjectionError> {
    super::arena_get(&projection.export.export_default_sources, source)
        .map(|_| ())
        .ok_or(
            CallableSourceParameterProjectionError::UnknownDefaultSource {
                position,
                source: super::raw_index(source),
            },
        )
}

fn unique_interface(
    export: &crate::ExportHir,
    owner: crate::ExportParameterOwner,
) -> Result<&ExportParameterInterface, CallableSourceParameterProjectionError> {
    let mut matches = export
        .source_parameter_interfaces
        .iter()
        .filter(|interface| interface.owner == owner);
    let Some(interface) = matches.next() else {
        return Err(CallableSourceParameterProjectionError::MissingInterface);
    };
    if matches.next().is_some() {
        return Err(CallableSourceParameterProjectionError::DuplicateInterface);
    }
    Ok(interface)
}
