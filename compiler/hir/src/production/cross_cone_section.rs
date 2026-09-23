//! Assembly of the complete general HIR interface payload.

use std::fmt;

use crate::{
    CallableInterfaceBuildError, CallableSourceInterfaceProductionError,
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalTypeAliasInterfacesV1,
    CrossConeHirInterfaceSectionV1, DefaultTemplateProductionError, DependencyHirOutput,
    ExportConstValueBuildError, ExportDefinitionSourceProductionError, ExportHir,
    ExternalHirBindingWitnessUse, ExternalHirReferenceProductionError,
    ExternalHirReferenceProductionInput, ExternalHirReferenceSemanticAuthority,
    NominalInterfaceBuildError, PropertyInterfaceBuildError, TypeAliasInterfaceBuildError,
};

impl CrossConeHirInterfaceSectionV1 {
    /// Projects a self-contained Export HIR graph into the complete general
    /// cross-Cone interface section.
    pub fn from_export_hir<A, E>(
        export: &ExportHir,
        witness_uses: &[ExternalHirBindingWitnessUse],
        authority: &mut A,
    ) -> Result<Self, CrossConeHirInterfaceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        Self::from_parts(export, None, witness_uses, authority)
    }

    /// Projects a current-Cone HIR graph using its dependency selections to
    /// serialize exported default bodies.
    pub fn from_dependency_hir<A, E>(
        output: &DependencyHirOutput,
        witness_uses: &[ExternalHirBindingWitnessUse],
        authority: &mut A,
    ) -> Result<Self, CrossConeHirInterfaceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut complete_witness_uses = Vec::with_capacity(
            witness_uses.len()
                + output.binding_witness_uses().len()
                + output.concrete_dependency_witness_uses().len(),
        );
        complete_witness_uses.extend_from_slice(witness_uses);
        complete_witness_uses.extend_from_slice(output.binding_witness_uses());
        complete_witness_uses.extend_from_slice(output.concrete_dependency_witness_uses());
        Self::from_parts(
            output.output().export.module(),
            Some(output.imported_dependencies()),
            &complete_witness_uses,
            authority,
        )
    }

    fn from_parts<A, E>(
        export: &ExportHir,
        imported_dependencies: Option<&crate::SelectedImportedDependencySet>,
        witness_uses: &[ExternalHirBindingWitnessUse],
        authority: &mut A,
    ) -> Result<Self, CrossConeHirInterfaceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut meter = scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default());
        let roots =
            super::nominal_interfaces::SharedSourceRoots::from_export_hir(export, &mut meter)
                .map_err(CrossConeHirInterfaceProductionError::Nominals)?;
        let nominal_interfaces = CanonicalNominalInterfacesV1::from_export_hir_with_source_roots(
            export, &roots, &mut meter,
        )
        .map_err(CrossConeHirInterfaceProductionError::Nominals)?;
        let property_interfaces = CanonicalPropertyInterfacesV1::from_export_hir_with_nominals(
            export,
            &nominal_interfaces,
            &roots,
            &mut meter,
        )
        .map_err(CrossConeHirInterfaceProductionError::Properties)?;
        let callable_interfaces = CanonicalCallableInterfacesV1::from_export_hir_with_nominals(
            export,
            &property_interfaces,
            &nominal_interfaces,
            &roots,
            &mut meter,
        )
        .map_err(CrossConeHirInterfaceProductionError::Callables)?;
        let type_aliases = CanonicalTypeAliasInterfacesV1::from_export_hir(export)
            .map_err(CrossConeHirInterfaceProductionError::TypeAliases)?;
        let source_interfaces =
            CanonicalCallableSourceInterfacesV1::from_export_hir_with_callables(
                export,
                &callable_interfaces,
            )
            .map_err(CrossConeHirInterfaceProductionError::SourceInterfaces)?;
        let default_templates = CanonicalExportDefaultTemplatesV1::from_parts_with_interfaces(
            export,
            imported_dependencies,
            &callable_interfaces,
            &source_interfaces,
        )
        .map_err(CrossConeHirInterfaceProductionError::Defaults)?;
        let constants = CanonicalExportConstValuesV1::from_export_hir(export)
            .map_err(CrossConeHirInterfaceProductionError::Constants)?;
        let definition_sources = CanonicalExportDefinitionSourcesV1::from_interface_parts(
            &type_aliases,
            &source_interfaces,
            &default_templates,
            &constants,
        )
        .map_err(CrossConeHirInterfaceProductionError::DefinitionSources)?;
        let external_references =
            CanonicalExternalHirReferencesV1::from_interface_parts_with_dependencies(
                ExternalHirReferenceProductionInput::new(
                    &export.public_export_bindings,
                    &nominal_interfaces,
                    &callable_interfaces,
                    &property_interfaces,
                    &type_aliases,
                    &source_interfaces,
                    &default_templates,
                    &constants,
                ),
                witness_uses,
                imported_dependencies,
                authority,
            )
            .map_err(CrossConeHirInterfaceProductionError::ExternalReferences)?;

        Ok(Self::new(
            export.public_export_bindings.clone(),
            nominal_interfaces,
            callable_interfaces,
            property_interfaces,
            type_aliases,
            source_interfaces,
            default_templates,
            constants,
            definition_sources,
            external_references,
        ))
    }
}

#[derive(Debug)]
pub enum CrossConeHirInterfaceProductionError<E> {
    Nominals(NominalInterfaceBuildError),
    Callables(CallableInterfaceBuildError),
    Properties(PropertyInterfaceBuildError),
    TypeAliases(TypeAliasInterfaceBuildError),
    SourceInterfaces(CallableSourceInterfaceProductionError),
    Defaults(DefaultTemplateProductionError),
    Constants(ExportConstValueBuildError),
    DefinitionSources(ExportDefinitionSourceProductionError),
    ExternalReferences(ExternalHirReferenceProductionError<E>),
}

impl<E: fmt::Display> fmt::Display for CrossConeHirInterfaceProductionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (section, source): (&str, &dyn fmt::Display) = match self {
            Self::Nominals(source) => ("nominal interfaces", source),
            Self::Callables(source) => ("callable interfaces", source),
            Self::Properties(source) => ("property interfaces", source),
            Self::TypeAliases(source) => ("type-alias interfaces", source),
            Self::SourceInterfaces(source) => ("source-call interfaces", source),
            Self::Defaults(source) => ("default templates", source),
            Self::Constants(source) => ("constant values", source),
            Self::DefinitionSources(source) => ("definition sources", source),
            Self::ExternalReferences(source) => ("external references", source),
        };
        write!(
            formatter,
            "cannot produce cross-Cone HIR {section}: {source}"
        )
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CrossConeHirInterfaceProductionError<E> {}
