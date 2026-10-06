use super::*;
use crate::{
    CrossConeHirConstSurfaceError, CrossConeHirDefinitionSourceSurfaceError,
    CrossConeHirSourceInterfaceSurfaceError, DefinitionSourceProviderView,
};
use scoop_hir::{
    ExportDefinitionSourceSemanticValidationError, ExportDefinitionSourceSetSemanticValidationError,
};

impl<'a> HirInterfaceValidationInput<'a> {
    pub(crate) fn definition_sources(
        self,
        dependencies: &[DefinitionSourceProviderView<'_>],
    ) -> Result<(), CrossConeHirDefinitionSourceSurfaceError> {
        let sources = self.interface.definition_sources().sources();
        let mut checked = std::collections::BTreeSet::new();

        for (index, source) in sources.iter().enumerate() {
            let provider = source.origin().source().cone();

            let foundation = if provider == self.current {
                self.foundation
            } else {
                dependencies
                    .iter()
                    .find(|dependency| dependency.identity == provider)
                    .map(|dependency| dependency.foundation)
                    .ok_or(
                        CrossConeHirDefinitionSourceSurfaceError::UnavailableProvider {
                            index,
                            provider,
                        },
                    )?
            };
            foundation
                .validate_definition_source_location(provider, source)
                .map_err(|error| {
                    CrossConeHirDefinitionSourceSurfaceError::DefinitionSources(
                        ExportDefinitionSourceSetSemanticValidationError::Source {
                            index,
                            error: ExportDefinitionSourceSemanticValidationError::Foundation(error),
                        },
                    )
                })?;
            checked.insert(scoop_identity::EvaluationOrigin::at_definition(
                source.origin(),
            ));
        }
        self.interface.visit_template_evaluation_origins(
            &mut |source| {
                if !checked.insert(source.clone()) {
                    return Ok(());
                }
                let provider = source.source().cone();
                let foundation = if provider == self.current {
                    self.foundation
                } else {
                    dependencies.iter().find(|dependency| dependency.identity == provider)
                        .map(|dependency| dependency.foundation)
                        .ok_or(CrossConeHirDefinitionSourceSurfaceError::UnavailableEvaluationProvider { provider })?
                };
                foundation.validate_evaluation_source_location(provider, source)
                    .map_err(|error| CrossConeHirDefinitionSourceSurfaceError::EvaluationOrigin {
                        origin: Box::new(source.clone()), error,
                    })
            },
            &scoop_wire::WirePath::root(),
        )?;
        Ok(())
    }
    pub(crate) fn sources(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'a>>,
    ) -> Result<(), CrossConeHirSourceInterfaceSurfaceError> {
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            self.current,
            self.identities,
            self.foundation,
            self.interface,
            dependencies,
        );
        self.interface
            .source_interfaces()
            .validate_semantics(self.interface.callable_interfaces(), &mut authority)
            .map_err(CrossConeHirSourceInterfaceSurfaceError::SourceInterfaces)?;
        authority
            .validate_default_root_origins()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultRootOrigin)?;
        authority
            .validate_default_provider_contracts()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultProviderContract)?;
        authority
            .validate_default_nested_identities()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultNestedIdentity)?;
        authority
            .validate_default_local_data_flow()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultDataFlow)?;
        authority
            .validate_shared_source_inventory()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::SourceInventory)?;
        Ok(())
    }
    pub(crate) fn constants(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'a>>,
    ) -> Result<(), CrossConeHirConstSurfaceError> {
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            self.current,
            self.identities,
            self.foundation,
            self.interface,
            dependencies,
        );
        self.interface
            .constants()
            .validate_semantics(&mut authority)
            .map_err(CrossConeHirConstSurfaceError::Constants)?;
        authority
            .validate_annotations()
            .map_err(CrossConeHirConstSurfaceError::Annotations)?;
        Ok(())
    }
}
