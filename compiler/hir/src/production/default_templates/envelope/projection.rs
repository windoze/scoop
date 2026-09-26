//! Portable default body data, independent of public lookup/access tables.
use super::super::{
    DefaultTemplateEnvelopeProjectionError, body, entities::DefaultEntityProjector,
    locals::TemplateLocalProjection,
};
use super::scope::provider_scope;
use crate::*;
use scoop_identity::{SignatureTypeKey, StructuralDefinitionPath};

#[derive(Debug)]
pub(in crate::production::default_templates) struct ProjectedDefaultBody<'a> {
    pub root: PersistentLexicalRootV1,
    pub path: StructuralDefinitionPath,
    pub locals: CanonicalTemplateLocalTableV1,
    pub body: ExportDefaultBodyV1,
    pub result: SignatureTypeKey,
    pub allows_suspend: CanonicalBooleanV1,
    pub type_parameters: CanonicalBinderUseListV1,
    pub receiver: OptionalTemplateReceiverV1,
    pub value_parameters: CanonicalTemplateValueParametersV1,
    pub definition_origin: ExportDefinitionSourceV1,
    pub provider_binders: Vec<HirSignatureBinder>,
    pub references: &'a ExportDefaultReferences,
}

pub(in crate::production::default_templates) fn project_body<'a>(
    export: &'a ExportHir,
    entities: &DefaultEntityProjector<'_>,
    target_binders: &[HirSignatureBinder],
    source_id: ExportDefaultSourceId,
) -> Result<ProjectedDefaultBody<'a>, DefaultTemplateEnvelopeProjectionError> {
    let source = super::super::arena_get(&export.export_default_sources, source_id).ok_or(
        DefaultTemplateEnvelopeProjectionError::UnknownDefaultSource(
            super::super::default_source_id(source_id),
        ),
    )?;
    let template = super::super::arena_get(&export.export_default_exprs, source.expression).ok_or(
        DefaultTemplateEnvelopeProjectionError::UnknownDefaultExpression(super::super::raw_index(
            source.expression,
        )),
    )?;
    let provider = provider_scope(export, entities, template.definition_root)?;
    if template.type_parameters.len() != provider.flattened.len() {
        return Err(DefaultTemplateEnvelopeProjectionError::TypeParameterArity {
            expected: provider.flattened.len(),
            actual: template.type_parameters.len(),
        });
    }
    if let Some(position) = template
        .type_parameters
        .iter()
        .zip(&provider.flattened)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(DefaultTemplateEnvelopeProjectionError::TypeParameterIdentity { position });
    }
    if source.type_arguments.len() != provider.flattened.len() {
        return Err(DefaultTemplateEnvelopeProjectionError::TypeParameterArity {
            expected: provider.flattened.len(),
            actual: source.type_arguments.len(),
        });
    }

    let type_parameters = source
        .type_arguments
        .iter()
        .map(|&argument| {
            entities
                .type_key(argument, target_binders)
                .map_err(DefaultTemplateEnvelopeProjectionError::Provider)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let type_parameters = CanonicalBinderUseListV1::try_new(type_parameters)
        .map_err(DefaultTemplateEnvelopeProjectionError::BinderUse)?;
    let (locals, local_table) =
        TemplateLocalProjection::project(entities, template, &provider.binders)?;
    let body = body::project(
        entities,
        &locals,
        &provider.binders,
        template.origin,
        &template.statements,
        &template.value,
    )
    .map_err(DefaultTemplateEnvelopeProjectionError::Body)?;
    let result = entities
        .type_key(template.result_type, &provider.binders)
        .map_err(DefaultTemplateEnvelopeProjectionError::Provider)?;
    let receiver = template
        .receiver
        .map(|receiver| {
            TemplateReceiverV1::try_new(
                locals
                    .selector(receiver.local)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Body)?,
                entities
                    .type_key(receiver.ty, &provider.binders)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Provider)?,
            )
            .map(OptionalTemplateReceiverV1::Present)
            .map_err(DefaultTemplateEnvelopeProjectionError::Receiver)
        })
        .transpose()?
        .unwrap_or(OptionalTemplateReceiverV1::Absent);

    let mut value_parameters = Vec::with_capacity(template.value_parameters.len());
    for (index, parameter) in template.value_parameters.iter().enumerate() {
        value_parameters.push(
            TemplateValueParameterV1::try_new(
                parameter.position,
                locals
                    .selector(parameter.local)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Body)?,
            )
            .map_err(|source| {
                DefaultTemplateEnvelopeProjectionError::ValueParameter { index, source }
            })?,
        );
    }
    let value_parameters = CanonicalTemplateValueParametersV1::try_new(value_parameters)
        .map_err(DefaultTemplateEnvelopeProjectionError::ValueParameters)?;

    let definition_origin =
        crate::production::definition_sources::project_definition_source(export, template.origin)
            .map_err(DefaultTemplateEnvelopeProjectionError::DefinitionOrigin)?;
    Ok(ProjectedDefaultBody {
        root: provider.root,
        path: template.definition_path.clone(),
        locals: local_table,
        body,
        result,
        allows_suspend: template.allows_suspend.into(),
        type_parameters,
        receiver,
        value_parameters,
        definition_origin,
        provider_binders: provider.binders,
        references: &template.references,
    })
}
