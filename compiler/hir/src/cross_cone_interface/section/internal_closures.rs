use std::fmt;

use scoop_wire::{BudgetMeter, WirePath};

use super::CrossConeHirInterfaceSectionV1;
use crate::{
    CallableDeclarationInventoryError, CanonicalDirectPublicSurfaceV1,
    ExportConstValueClosureValidationError, ExportDefaultTemplateSourceClosureValidationError,
    ExportDefinitionSourceClosureValidationError, PropertyAccessorClosureValidationError,
    PublicExportBindingDirectSurfaceValidationError,
};

impl CrossConeHirInterfaceSectionV1 {
    /// Validates every exact relationship that can be reconstructed entirely
    /// from this section plus the already validated foundation direct surface.
    /// Cross-artifact identity, route, and visibility authority remain the
    /// responsibility of the closure-wide semantic pass.
    pub fn validate_internal_closures(
        &self,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), CrossConeHirInternalClosureValidationError> {
        self.public_bindings()
            .validate_direct_surface(direct_surface)
            .map_err(CrossConeHirInternalClosureValidationError::DirectSurface)?;
        self.property_interfaces()
            .validate_declaration_inventory(self.nominal_interfaces(), meter)
            .map_err(CrossConeHirInternalClosureValidationError::PropertyDeclarations)?;
        self.callable_interfaces()
            .validate_declaration_inventory(
                self.nominal_interfaces(),
                self.property_interfaces(),
                meter,
            )
            .map_err(CrossConeHirInternalClosureValidationError::CallableDeclarations)?;
        self.property_interfaces()
            .validate_accessor_closure_with_budget(self.callable_interfaces(), meter)
            .map_err(CrossConeHirInternalClosureValidationError::PropertyAccessors)?;
        self.default_templates()
            .validate_source_closure(self.source_interfaces())
            .map_err(CrossConeHirInternalClosureValidationError::DefaultTemplates)?;
        self.constants()
            .validate_property_closure(self.property_interfaces())
            .map_err(CrossConeHirInternalClosureValidationError::Constants)?;
        self.validate_definition_source_closure(meter, path)
            .map_err(CrossConeHirInternalClosureValidationError::DefinitionSources)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CrossConeHirInternalClosureValidationError {
    DirectSurface(PublicExportBindingDirectSurfaceValidationError),
    CallableDeclarations(CallableDeclarationInventoryError),
    PropertyDeclarations(crate::PropertyDeclarationInventoryError),
    PropertyAccessors(PropertyAccessorClosureValidationError),
    DefaultTemplates(ExportDefaultTemplateSourceClosureValidationError),
    Constants(ExportConstValueClosureValidationError),
    DefinitionSources(ExportDefinitionSourceClosureValidationError),
}

impl fmt::Display for CrossConeHirInternalClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (relation, error): (&str, &dyn fmt::Display) = match self {
            Self::DirectSurface(error) => ("direct public surface", error),
            Self::CallableDeclarations(error) => ("callable declaration inventory", error),
            Self::PropertyDeclarations(error) => ("property declaration inventory", error),
            Self::PropertyAccessors(error) => ("property accessor closure", error),
            Self::DefaultTemplates(error) => ("default template closure", error),
            Self::Constants(error) => ("constant closure", error),
            Self::DefinitionSources(error) => ("definition source closure", error),
        };
        write!(formatter, "invalid cross-Cone HIR {relation}: {error}")
    }
}

impl std::error::Error for CrossConeHirInternalClosureValidationError {}
