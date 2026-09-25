//! Artifact locations for the complete nominal default source inventory.
use crate::*;
use scoop_identity::ConeIdentity;
use scoop_wire::WirePath;

mod errors;
mod providers;
pub use errors::*;
type Error = DefaultSourceOriginBindingError;

/// Parameter coverage and artifact locations only. Provider paths, inherited
/// relations, source profiles, body semantics and access remain separate proofs.
#[derive(Debug)]
pub struct BoundNominalDefaultOriginsV1<'d, 'p, 's, 'a, 'f> {
    parameters: &'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>,
    templates: &'d CanonicalDefaultSourceTemplatesV1,
}

impl<'p, 's, 'a, 'f> BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
    pub fn bind_default_origins<'d>(
        &'d self,
        templates: &'d CanonicalDefaultSourceTemplatesV1,
        dependencies: &[&BoundTypeFoundationSourcesV1<'_>],
    ) -> Result<BoundNominalDefaultOriginsV1<'d, 'p, 's, 'a, 'f>, Error> {
        let path = WirePath::root();

        templates.validate_parameter_coverage(self.table())?;
        let current = self.members().nominals.foundation;
        let providers = providers::Providers::new(current, dependencies)?;
        for (index, template) in templates.records().iter().enumerate() {
            let path = path.clone().index(index as u64);
            let key = template.key();
            let origin = template.definition_origin();
            let provider = providers.get(origin.origin().source().cone())?;
            provider
                .foundation
                .validate_default_template_root_origin(
                    provider.source().entries().provider,
                    provider.identities,
                    template.definition_root(),
                    template.definition_origin(),
                )
                .map_err(Error::Root)?;
            template.visit_definition_sources(
                &mut |origin: &ExportDefinitionSourceV1, _, _path: &WirePath| {
                    providers
                        .get(origin.origin().source().cone())?
                        .validate_origin(origin)
                        .map_err(|error| Error::Origin { key, error })
                },
                &path,
            )?;
        }
        Ok(BoundNominalDefaultOriginsV1 {
            parameters: self,
            templates,
        })
    }
}

impl<'d, 'p, 's, 'a, 'f> BoundNominalDefaultOriginsV1<'d, 'p, 's, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.parameters.provider()
    }
    pub const fn parameters(&self) -> &'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
        self.parameters
    }
    pub const fn templates(&self) -> &'d CanonicalDefaultSourceTemplatesV1 {
        self.templates
    }
}
