//! Default root locations resolved through the ordinary artifact closure.

use scoop_hir::{DefaultTemplateRootOriginValidationError, ExportDefaultTemplateKeyV1};
use scoop_identity::ConeIdentity;
use scoop_wire::{WireError, WirePath};

use super::CanonicalCrossConeHirSurfaceAuthority;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_root_origins(
        &mut self,
    ) -> Result<(), CrossConeHirDefaultRootOriginError> {
        let path = WirePath::root().field(7);
        let templates = self.current_interface.default_templates().records();
        self.meter
            .check_table_entries(templates.len() as u64, &path)?;
        self.meter.charge_work(templates.len() as u64, &path)?;
        for (index, template) in templates.iter().enumerate() {
            let path = path.clone().index(index as u64);
            let provider = template.definition_origin().origin().source().cone();
            let (identities, foundation) = if provider == self.current {
                (self.identities, self.current_foundation)
            } else {
                self.meter
                    .charge_work(self.dependencies.len() as u64, &path)?;
                let dependency = self
                    .dependencies
                    .iter()
                    .find(|dependency| dependency.identity == provider)
                    .ok_or(CrossConeHirDefaultRootOriginError::UnreachableProvider {
                        index,
                        key: template.key(),
                        provider,
                    })?;
                (dependency.identities, dependency.foundation)
            };
            foundation
                .validate_default_template_root_origin(
                    provider,
                    identities,
                    template.definition_root(),
                    template.definition_origin(),
                    self.meter,
                    &path,
                )
                .map_err(|source| CrossConeHirDefaultRootOriginError::Template {
                    index,
                    key: template.key(),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum CrossConeHirDefaultRootOriginError {
    Resource(WireError),
    UnreachableProvider {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        provider: ConeIdentity,
    },
    Template {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        source: Box<DefaultTemplateRootOriginValidationError>,
    },
}

impl From<WireError> for CrossConeHirDefaultRootOriginError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for CrossConeHirDefaultRootOriginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::UnreachableProvider {
                index,
                key,
                provider,
            } => write!(
                f,
                "default template[{index}] {key:?} has unreachable source provider {provider:?}"
            ),
            Self::Template { index, key, source } => write!(
                f,
                "default template[{index}] {key:?} has an invalid root origin: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeHirDefaultRootOriginError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::UnreachableProvider { .. } => None,
            Self::Template { source, .. } => Some(source.as_ref()),
        }
    }
}
