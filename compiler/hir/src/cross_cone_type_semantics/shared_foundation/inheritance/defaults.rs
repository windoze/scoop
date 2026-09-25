//! Bind restricted default projections to the already validated shared body.

use super::*;
use crate::{ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1, ProtectedDefaultTemplateKeyV1};
use scoop_identity::ConeIdentity;

mod origins;
mod references;
mod witnesses;

pub(super) fn validate(
    provider: CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[CheckedSharedTypeFoundationV1<'_>],
    context: &Context<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
) -> Result<(), Error> {
    let shared = provider.metadata.public.default_templates();
    for (index, template) in provider
        .section
        .protected_defaults()
        .records()
        .iter()
        .enumerate()
    {
        let key = template.key();
        let path = WirePath::root().field(6).index(index as u64);

        let expected = shared
            .get(ExportDefaultTemplateKeyV1::new(
                key.owner(),
                key.parameter_position(),
            ))
            .ok_or(Error::DefaultContract(key))?;
        if !template.matches_shared_template(expected, &path)? {
            return Err(Error::DefaultContract(key));
        }
        references::inventory(template.references(), expected.references(), key)?;
        let witnesses = witnesses::Witnesses::new(
            provider.metadata,
            dependencies,
            context,
            graph,
            key,
            expected,
        )?;
        template
            .references()
            .validate_body_closure(
                key,
                expected.body(),
                expected.locals(),
                expected.definition_origin(),
                expected.receiver(),
                &mut references::Replay {
                    key,
                    shared: expected.references(),
                    witnesses,
                },
                &path,
            )
            .map_err(|error| Error::DefaultBody(Box::new(error)))?;
    }
    provider
        .section
        .definition_source_inputs()
        .validate_definition_sources(
            provider.section.definition_sources(),
            &mut origins::Replay {
                metadata: provider.metadata,
                dependencies,
            },
            &WirePath::root(),
        )
        .map_err(|error| Error::DefinitionSources(Box::new(error)))
}

fn metadata<'a>(
    current: SharedTypeMetadataV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    provider: ConeIdentity,
) -> Result<SharedTypeMetadataV1<'a>, Error> {
    if current.provider == provider {
        return Ok(current);
    }
    dependencies
        .iter()
        .find(|record| record.provider() == provider)
        .map(|record| record.metadata())
        .ok_or(Error::MissingProvider(provider))
}
