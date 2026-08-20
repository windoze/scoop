use super::*;
use crate::{ExportReleaseHookRef, ExportReleaseTemplateV1, ReleasePolicy};

pub(super) fn project(
    entities: &DefaultEntityProjector<'_>,
    declaration: &crate::ClassDecl,
) -> Result<ReleasePolicy<ExportReleaseTemplateV1>, Error> {
    let id = match declaration.release_policy {
        ReleasePolicy::None => return Ok(ReleasePolicy::None),
        ReleasePolicy::SynchronousGcFree {
            hook: ExportReleaseHookRef::Template(id),
        } => id,
        ReleasePolicy::SynchronousGcFree {
            hook: ExportReleaseHookRef::Imported { .. },
        } => {
            return Err(Error::Initialization(
                crate::GenericInitializationBuildError::ReleaseOwner,
            ));
        }
    };
    let export = entities.export();
    let hook = &export.release_hooks[id];
    let binders = crate::production::signatures::HirInterfaceSignatureProjector::new(export)
        .binder_frame(&declaration.type_params, 0)
        .map_err(Error::Signature)?;
    let (locals, local_table) = super::super::locals::TemplateLocalProjection::project(
        entities,
        &hook.body.locals,
        &binders,
    )
    .map_err(|source| Error::Locals(Box::new(source)))?;
    let body = super::super::body::project_fragment(
        entities,
        &locals,
        &binders,
        hook.origin,
        &hook.body.statements,
        &[],
        local_table,
    )
    .map_err(|source| Error::Body(Box::new(source)))?;
    Ok(ReleasePolicy::SynchronousGcFree {
        hook: ExportReleaseTemplateV1::try_new(
            fragment::definition_source(export, hook.origin)?,
            body,
        )
        .map_err(Error::Initialization)?,
    })
}
