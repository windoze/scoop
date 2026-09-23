use super::*;
use crate::{
    CrossConeHirConstSurfaceError, CrossConeHirDefinitionSourceSurfaceError,
    CrossConeHirSourceInterfaceSurfaceError, DefinitionSourceProviderView,
};
use scoop_hir::{
    ExportDefinitionSourceSemanticValidationError, ExportDefinitionSourceSetSemanticValidationError,
};
use scoop_wire::WirePath;

impl<'a> HirInterfaceValidationInput<'a> {
    pub(crate) fn definition_sources(
        self,
        dependencies: &[DefinitionSourceProviderView<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<(), CrossConeHirDefinitionSourceSurfaceError> {
        let path = WirePath::root().field(9);
        let sources = self.interface.definition_sources().sources();
        meter
            .check_table_entries(sources.len() as u64, &path)
            .map_err(CrossConeHirDefinitionSourceSurfaceError::Resource)?;
        for (index, source) in sources.iter().enumerate() {
            let path = path.clone().index(index as u64);
            let provider = source.origin().source().cone();
            meter
                .charge_work(
                    (dependencies.len() as u64)
                        .saturating_mul(32)
                        .saturating_add(1),
                    &path,
                )
                .map_err(CrossConeHirDefinitionSourceSurfaceError::Resource)?;
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
                .validate_definition_source_location(provider, source, meter, &path)
                .map_err(|error| {
                    CrossConeHirDefinitionSourceSurfaceError::DefinitionSources(
                        ExportDefinitionSourceSetSemanticValidationError::Source {
                            index,
                            error: ExportDefinitionSourceSemanticValidationError::Foundation(error),
                        },
                    )
                })?;
        }
        Ok(())
    }
    pub(crate) fn sources(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'a>>,
        meter: &'a mut BudgetMeter,
    ) -> Result<(), CrossConeHirSourceInterfaceSurfaceError> {
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            self.current,
            self.identities,
            self.foundation,
            self.interface,
            dependencies,
            meter,
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
            .validate_default_type_access(self.core)
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultTypeAccess)?;
        authority
            .validate_default_value_access()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultValueAccess)?;
        authority
            .validate_default_callable_access(self.core)
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultCallableAccess)?;
        authority
            .validate_default_call_domains()
            .map_err(CrossConeHirSourceInterfaceSurfaceError::DefaultCallDomain)?;
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
        meter: &'a mut BudgetMeter,
    ) -> Result<(), CrossConeHirConstSurfaceError> {
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            self.current,
            self.identities,
            self.foundation,
            self.interface,
            dependencies,
            meter,
        );
        self.interface
            .constants()
            .validate_semantics(&mut authority)
            .map_err(CrossConeHirConstSurfaceError::Constants)?;
        Ok(())
    }
}
