//! Source reference envelopes around the shared portable default body projection.
use super::{
    entities::DefaultEntityProjector, errors::DefaultTemplateEnvelopeProjectionError, references,
};
use crate::{
    CanonicalCallableInterfacesV1, ExportDefaultSource, ExportDefaultSourceId,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
};

pub(super) mod projection;
pub(super) mod scope;

pub(super) fn project(
    entities: &DefaultEntityProjector<'_>,
    callables: &CanonicalCallableInterfacesV1,
    owner: &super::SourceCallableOwner,
    position: u32,
    source_id: ExportDefaultSourceId,
) -> Result<ExportDefaultTemplateV1, super::DefaultTemplateProductionError> {
    let key = ExportDefaultTemplateKeyV1::new(owner.declaration, position);
    project_inner(entities, callables, owner, key, source_id).map_err(|source| {
        super::DefaultTemplateProductionError::Template {
            key,
            source: Box::new(source),
        }
    })
}

fn project_inner(
    entities: &DefaultEntityProjector<'_>,
    callables: &CanonicalCallableInterfacesV1,
    owner: &super::SourceCallableOwner,
    key: ExportDefaultTemplateKeyV1,
    source_id: ExportDefaultSourceId,
) -> Result<ExportDefaultTemplateV1, DefaultTemplateEnvelopeProjectionError> {
    let result = project_template(entities, &owner.binders, key, source_id)?;
    if matches!(
        entities.inputs().export_default_sources[source_id],
        ExportDefaultSource::Declared { .. }
    ) {
        for declaration in [owner.declaration, result.definition_root().declaration()] {
            callables.declaration(declaration).ok_or(
                DefaultTemplateEnvelopeProjectionError::Provider(
                    super::DefaultEntityProjectionError::MissingIdentity {
                        kind: "default provider callable interface",
                        index: super::owner_index(owner.local),
                    },
                ),
            )?;
        }
    }
    Ok(result)
}

pub(super) fn project_template(
    entities: &DefaultEntityProjector<'_>,
    target_binders: &[crate::HirSignatureBinder],
    key: ExportDefaultTemplateKeyV1,
    source_id: ExportDefaultSourceId,
) -> Result<ExportDefaultTemplateV1, DefaultTemplateEnvelopeProjectionError> {
    let source = super::arena_get(entities.inputs().export_default_sources, source_id).ok_or(
        DefaultTemplateEnvelopeProjectionError::UnknownDefaultSource(super::default_source_id(
            source_id,
        )),
    )?;
    let (expression, type_arguments) = match source {
        ExportDefaultSource::Declared {
            expression,
            type_arguments,
        } => (*expression, type_arguments),
        ExportDefaultSource::Imported {
            template,
            type_arguments,
        } => {
            let arguments = type_arguments
                .iter()
                .map(|&argument| {
                    entities
                        .type_key(argument, target_binders)
                        .map_err(DefaultTemplateEnvelopeProjectionError::Provider)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let parameters = crate::CanonicalBinderUseListV1::try_new(arguments)
                .map_err(DefaultTemplateEnvelopeProjectionError::BinderUse)?;
            return Ok(template.as_ref().clone().inherited_at(key, parameters));
        }
    };
    let projected = projection::project_body(entities, target_binders, expression, type_arguments)?;
    let references =
        references::project(entities, &projected.provider_binders, projected.references)
            .map_err(DefaultTemplateEnvelopeProjectionError::References)?;
    ExportDefaultTemplateV1::try_new(
        key,
        projected.root,
        projected.path,
        projected.locals,
        projected.body,
        projected.result,
        projected.allows_suspend,
        projected.type_parameters,
        projected.receiver,
        projected.value_parameters,
        references,
        projected.definition_origin,
    )
    .map_err(DefaultTemplateEnvelopeProjectionError::Record)
}
