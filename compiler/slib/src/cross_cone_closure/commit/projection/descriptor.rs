//! Source nominal descriptors use the shared artifact and shape-support lookup.

use scoop_identity::{ConeIdentity, ExactTypeKey, PersistentExactTypeId, PersistentTypeId};
use scoop_lir::{ExternalTypeDescriptor, ImportedLirTypeDescriptorProjectionError};

use super::ValidatedCrossConeSemanticClosure;

impl ValidatedCrossConeSemanticClosure {
    /// Resolves the source key before selecting its actual defining provider.
    pub fn project_source_type_descriptor(
        &self,
        provider: ConeIdentity,
        nominal: PersistentTypeId,
    ) -> Result<ExternalTypeDescriptor, CrossConeTypeDescriptorProjectionError> {
        let artifact = self
            .provider(provider)
            .ok_or(Error::MissingProvider(provider))?;
        let source = artifact
            .hir()
            .source_nominal(nominal)
            .ok_or(Error::MissingSourceNominal { provider, nominal })?;
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source.persistent()))
            .map_err(Error::Identity)?;
        self.project_type_descriptor(source.provider(), exact)
    }

    /// A descriptor must agree with the provider's complete published support.
    pub fn project_type_descriptor(
        &self,
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    ) -> Result<ExternalTypeDescriptor, CrossConeTypeDescriptorProjectionError> {
        let artifact = self
            .provider(provider)
            .ok_or(Error::MissingProvider(provider))?;
        let production = artifact.production().lir_strong();
        let shapes = production.shape_support_plan().closures();
        let shape = shapes
            .binary_search_by_key(&exact, |shape| shape.owner())
            .ok()
            .map(|index| &shapes[index])
            .ok_or(Error::MissingShapeSupport { provider, exact })?;
        let descriptor = shape
            .roles()
            .type_descriptor()
            .available()
            .ok_or(Error::MissingShapeSupport { provider, exact })?;
        if descriptor.semantic_id() != exact {
            return Err(Error::ShapeSupportMismatch { provider, exact });
        }
        let projected = artifact
            .lir()
            .project_type_descriptor(production.canonical_definitions(), exact)
            .map_err(Error::Descriptor)?;
        if projected.expected_symbol() != descriptor.symbol()
            || projected.required_definition() != descriptor.definition_plan()
        {
            return Err(Error::ShapeSupportMismatch { provider, exact });
        }
        Ok(projected)
    }
}

type Error = CrossConeTypeDescriptorProjectionError;

#[derive(Debug)]
pub enum CrossConeTypeDescriptorProjectionError {
    MissingProvider(ConeIdentity),
    MissingSourceNominal {
        provider: ConeIdentity,
        nominal: PersistentTypeId,
    },
    MissingShapeSupport {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
    ShapeSupportMismatch {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
    Identity(scoop_wire::HashError),
    Descriptor(ImportedLirTypeDescriptorProjectionError),
}

impl std::fmt::Display for CrossConeTypeDescriptorProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProvider(provider) => write!(
                formatter,
                "descriptor provider {provider} is outside the committed dependency closure"
            ),
            Self::MissingSourceNominal { provider, nominal } => write!(
                formatter,
                "provider {provider} has no source nominal {nominal:?}"
            ),
            Self::MissingShapeSupport { provider, exact } => write!(
                formatter,
                "provider {provider} has no complete descriptor shape support for {exact:?}"
            ),
            Self::ShapeSupportMismatch { provider, exact } => write!(
                formatter,
                "provider {provider} descriptor for {exact:?} disagrees with its shape support"
            ),
            Self::Identity(error) => error.fmt(formatter),
            Self::Descriptor(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeTypeDescriptorProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::MissingProvider(_)
            | Self::MissingSourceNominal { .. }
            | Self::MissingShapeSupport { .. }
            | Self::ShapeSupportMismatch { .. } => None,
        }
    }
}
