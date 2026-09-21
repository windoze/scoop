//! Independent body projection for a selected source parameter, at any visibility.

use super::{
    entities::DefaultEntityProjector,
    envelope::{projection, scope},
};
use crate::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};

mod accessors;
mod errors;
pub use errors::DefaultSourceBodyProductionError;

/// A production intermediate, not a persisted source authority or checked template.
/// Retains both raw source references and their independently projected occurrences.
#[derive(Debug)]
pub struct DefaultSourceBodyProductionV1<'a> {
    owner: CallableTemplateOrigin,
    parameter_position: u32,
    projected: projection::ProjectedDefaultBody<'a>,
    references: DefaultSourceReferencesV1,
}

impl<'a> DefaultSourceBodyProductionV1<'a> {
    pub fn from_export_hir(
        export: &'a ExportHir,
        owner: ExportParameterOwner,
        parameter_position: u32,
        meter: &mut BudgetMeter,
    ) -> Result<Self, DefaultSourceBodyProductionError> {
        let entities = DefaultEntityProjector::new(export, None, meter);
        Self::project(export, &entities, owner, parameter_position)
    }

    pub fn from_dependency_hir(
        output: &'a DependencyHirOutput,
        owner: ExportParameterOwner,
        parameter_position: u32,
        meter: &mut BudgetMeter,
    ) -> Result<Self, DefaultSourceBodyProductionError> {
        let export = output.output().export.module();
        let entities =
            DefaultEntityProjector::new(export, Some(output.imported_dependencies()), meter);
        Self::project(export, &entities, owner, parameter_position)
    }

    fn project(
        export: &'a ExportHir,
        entities: &DefaultEntityProjector<'_, '_>,
        owner: ExportParameterOwner,
        parameter_position: u32,
    ) -> Result<Self, DefaultSourceBodyProductionError> {
        entities.resources.with_meter(|meter, _| {
            let path = WirePath::root();
            meter.check_table_entries(export.source_parameter_interfaces.len() as u64, &path)?;
            meter.charge_work(export.source_parameter_interfaces.len() as u64, &path)
        })?;
        let mut sources = export
            .source_parameter_interfaces
            .iter()
            .filter(|source| source.owner == owner);
        let source = sources
            .next()
            .ok_or(DefaultSourceBodyProductionError::MissingInterface(owner))?;
        if sources.next().is_some() {
            return Err(DefaultSourceBodyProductionError::DuplicateInterface(owner));
        }
        let parameter = source.parameters.get(parameter_position as usize).ok_or(
            DefaultSourceBodyProductionError::MissingParameter {
                owner,
                position: parameter_position,
            },
        )?;
        let source = match parameter.calling {
            ExportParameterCalling::Default { source, .. }
            | ExportParameterCalling::Vararg {
                omission: ExportVarargOmission::Default(source),
                ..
            } => source,
            ExportParameterCalling::Required { .. }
            | ExportParameterCalling::Vararg {
                omission: ExportVarargOmission::EmptyArray,
                ..
            } => {
                return Err(DefaultSourceBodyProductionError::NoDefault {
                    owner,
                    position: parameter_position,
                });
            }
        };
        let root = match owner {
            ExportParameterOwner::Function(id) => LexicalDefinitionRoot::Function(id),
            ExportParameterOwner::StructConstructor(id) => {
                LexicalDefinitionRoot::StructConstructor(id)
            }
            ExportParameterOwner::ClassConstructor(id) => {
                LexicalDefinitionRoot::ClassConstructor(id)
            }
            ExportParameterOwner::VariantConstructor(id) => {
                LexicalDefinitionRoot::VariantConstructor(id)
            }
        };
        let scope = scope::provider_scope(export, entities, root)
            .map_err(DefaultSourceBodyProductionError::Scope)?;
        let projected = projection::project_body(export, entities, &scope.binders, source)
            .map_err(DefaultSourceBodyProductionError::Body)?;
        let references = super::source_references::project(
            entities,
            projected.root.declaration(),
            &projected.provider_binders,
            projected.references,
        )
        .map_err(DefaultSourceBodyProductionError::References)?;
        Ok(Self {
            references,
            owner: scope.root.declaration(),
            parameter_position,
            projected,
        })
    }
}
