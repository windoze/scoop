use scoop_identity::{CanonicalIdentifier, SignatureTypeKey};

use super::{CallableProjection, SourceParameterProjectionError};
use crate::{
    CanonicalSourceParameterShapesV1, ExportParameterCalling, ExportParameterInterface,
    ExportParameterOwner, HirSignatureBinder, SourceParameterShapeV1, TypeId,
};

pub(super) fn project(
    projection: &CallableProjection<'_>,
    owner: ExportParameterOwner,
    binders: &[HirSignatureBinder],
    expected_types: &[SignatureTypeKey],
) -> Result<CanonicalSourceParameterShapesV1, SourceParameterProjectionError> {
    let source = unique_interface(projection.export, owner)?;
    if source.parameters.len() != expected_types.len() {
        return Err(SourceParameterProjectionError::Arity {
            expected: expected_types.len(),
            actual: source.parameters.len(),
        });
    }

    let mut parameters = Vec::with_capacity(source.parameters.len());
    for (index, (parameter, expected)) in source.parameters.iter().zip(expected_types).enumerate() {
        let position = u32::try_from(index).map_err(|_| {
            SourceParameterProjectionError::List(crate::SourceParameterListBuildError::TooMany)
        })?;
        let name = CanonicalIdentifier::new(&parameter.name)
            .map_err(|source| SourceParameterProjectionError::InvalidName { position, source })?;
        let value_type = calling_value_type(projection, position, parameter.calling)?;
        let actual = projection
            .signatures
            .map_type(value_type, binders)
            .map_err(|source| SourceParameterProjectionError::Signature { position, source })?;
        if &actual != expected {
            return Err(SourceParameterProjectionError::TypeMismatch {
                position,
                expected: Box::new(expected.clone()),
                actual: Box::new(actual),
            });
        }
        parameters.push(SourceParameterShapeV1::new(name, expected.clone()));
    }
    CanonicalSourceParameterShapesV1::try_new(parameters)
        .map_err(SourceParameterProjectionError::List)
}

fn unique_interface(
    export: &crate::ExportHir,
    owner: ExportParameterOwner,
) -> Result<&ExportParameterInterface, SourceParameterProjectionError> {
    let mut matches = export
        .source_parameter_interfaces
        .iter()
        .filter(|interface| interface.owner == owner);
    let Some(interface) = matches.next() else {
        return Err(SourceParameterProjectionError::MissingInterface);
    };
    if matches.next().is_some() {
        return Err(SourceParameterProjectionError::DuplicateInterface);
    }
    Ok(interface)
}

fn calling_value_type(
    projection: &CallableProjection<'_>,
    position: u32,
    calling: ExportParameterCalling,
) -> Result<TypeId, SourceParameterProjectionError> {
    match calling {
        ExportParameterCalling::Required { value_type }
        | ExportParameterCalling::Default { value_type, .. } => Ok(value_type),
        ExportParameterCalling::Vararg { parameter_type, .. } => super::arena_get(
            &projection.export.export_vararg_parameter_types,
            parameter_type,
        )
        .map(|parameter| parameter.array_type)
        .ok_or(SourceParameterProjectionError::UnknownVarargType {
            position,
            parameter_type: super::raw_index(parameter_type),
        }),
    }
}
