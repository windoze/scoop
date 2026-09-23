use super::*;
use scoop_identity::ExactTypeKey;
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

mod default_sources;

impl CanonicalHirFoundation {
    /// M23-6 source authority needs the exact type of every generated nominal
    /// in the same sealed HIR, including an otherwise unused object backing
    /// class, and all source parameter/default-body origins before materialization.
    /// Ordinary source identities are supplied by the shared foundation projection.
    pub fn from_type_semantics_output(
        output: &crate::DependencyHirOutput,
    ) -> Result<Self, HirFoundationBuildError> {
        Self::from_type_semantics_output_with_budget(
            output,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
    }

    /// Shares the source-default projection and occurrence budget with the caller.
    pub fn from_type_semantics_output_with_budget(
        output: &crate::DependencyHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<Self, HirFoundationBuildError> {
        let mut foundation = Self::from_dependency_output_with_budget(output, meter)?;
        let mut exacts = foundation
            .exact_types
            .iter()
            .map(|record| (record.id(), record.clone()))
            .collect::<BTreeMap<_, _>>();
        for generated in &foundation.generated_types {
            let key = ExactTypeKey::Nominal(generated.id());
            let exact = CborIdentityRecord::from_key(key).map_err(|error| {
                HirFoundationBuildError::IdentityDerivation {
                    table: HirFoundationTable::ExactType,
                    reason: error.to_string(),
                }
            })?;
            insert_identity(&mut exacts, &exact, HirFoundationTable::ExactType)?;
        }
        foundation.set_exact_types(exacts.into_values().collect())?;
        let export = &output.output().export;
        let mut parameters = export
            .source_parameter_interfaces
            .iter()
            .flat_map(|interface| &interface.parameters)
            .map(|parameter| {
                crate::production::project_definition_source(export, parameter.origin)
                    .map_err(HirFoundationBuildError::SourceParameterOrigin)
            })
            .collect::<Result<Vec<_>, _>>()?;
        default_sources::collect(output, &mut parameters, meter)?;
        foundation.set_sources(source_records(
            &export.source_files,
            &foundation.definition_origins,
            &parameters,
            &foundation.sources,
        )?)?;
        Ok(foundation)
    }
}
