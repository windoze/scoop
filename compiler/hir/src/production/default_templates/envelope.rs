//! Source reference envelopes around the shared portable default body projection.
use super::{
    entities::DefaultEntityProjector, errors::DefaultTemplateEnvelopeProjectionError, references,
};
use crate::{
    CanonicalCallableInterfacesV1, ExportDefaultSourceId, ExportDefaultTemplateKeyV1,
    ExportDefaultTemplateV1, ExportHir,
};

pub(super) mod projection;
pub(super) mod scope;

pub(super) fn project(
    export: &ExportHir,
    entities: &DefaultEntityProjector<'_>,
    callables: &CanonicalCallableInterfacesV1,
    owner: &super::SourceCallableOwner,
    position: u32,
    source_id: ExportDefaultSourceId,
) -> Result<ExportDefaultTemplateV1, super::DefaultTemplateProductionError> {
    let key = ExportDefaultTemplateKeyV1::new(owner.declaration, position);
    project_inner(export, entities, callables, owner, key, source_id).map_err(|source| {
        super::DefaultTemplateProductionError::Template {
            key,
            source: Box::new(source),
        }
    })
}

fn project_inner(
    export: &ExportHir,
    entities: &DefaultEntityProjector<'_>,
    callables: &CanonicalCallableInterfacesV1,
    owner: &super::SourceCallableOwner,
    key: ExportDefaultTemplateKeyV1,
    source_id: ExportDefaultSourceId,
) -> Result<ExportDefaultTemplateV1, DefaultTemplateEnvelopeProjectionError> {
    let projected = projection::project_body(export, entities, &owner.binders, source_id)?;
    callables.declaration(owner.declaration).ok_or(
        DefaultTemplateEnvelopeProjectionError::Provider(
            super::DefaultEntityProjectionError::MissingIdentity {
                kind: "default owner callable interface",
                index: super::owner_index(owner.local),
            },
        ),
    )?;
    let provider_owner = projected.root.declaration();
    callables.declaration(provider_owner).ok_or(
        DefaultTemplateEnvelopeProjectionError::Provider(
            super::DefaultEntityProjectionError::MissingIdentity {
                kind: "default provider callable interface",
                index: super::owner_index(owner.local),
            },
        ),
    )?;
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
