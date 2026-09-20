//! Public reference envelopes around the shared portable default body projection.
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
    entities: &DefaultEntityProjector<'_, '_, '_>,
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
    entities: &DefaultEntityProjector<'_, '_, '_>,
    callables: &CanonicalCallableInterfacesV1,
    owner: &super::SourceCallableOwner,
    key: ExportDefaultTemplateKeyV1,
    source_id: ExportDefaultSourceId,
) -> Result<ExportDefaultTemplateV1, DefaultTemplateEnvelopeProjectionError> {
    let projected = projection::project_body(export, entities, &owner.binders, source_id)?;
    let owner_interface = callables.get(owner.declaration).ok_or(
        DefaultTemplateEnvelopeProjectionError::Provider(
            super::DefaultEntityProjectionError::MissingIdentity {
                kind: "default owner callable interface",
                index: super::owner_index(owner.local),
            },
        ),
    )?;
    let provider_owner = projected.root.declaration();
    let provider_interface =
        callables
            .get(provider_owner)
            .ok_or(DefaultTemplateEnvelopeProjectionError::Provider(
                super::DefaultEntityProjectionError::MissingIdentity {
                    kind: "default provider callable interface",
                    index: super::owner_index(owner.local),
                },
            ))?;
    let reference_projection = references::ReferenceProjection {
        entities,
        source_owner: provider_owner,
        source_access: provider_interface.access(),
        target_owner: owner.declaration,
        target_access: owner_interface.access(),
        binders: &projected.provider_binders,
    };
    let references = references::project(&reference_projection, projected.references)
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
