//! Projection of declaration-bound HIR defaults into portable templates.

use crate::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportDefaultTemplatesV1, ExportDefaultSourceId, ExportHir, ExportParameterCalling,
    ExportParameterInterface, ExportParameterOwner, OrdinaryHirOutput, SelectedImportedCoreSet,
};

use super::callable_source_interfaces::{SourceCallableOwner, public_source_callable_owners};

mod body;
mod entities;
mod envelope;
mod errors;
mod locals;
mod references;

pub use errors::{
    DefaultBodyProjectionError, DefaultEntityProjectionError, DefaultReferenceProjectionError,
    DefaultTemplateEnvelopeProjectionError, DefaultTemplateProductionError,
};

use entities::DefaultEntityProjector;

impl CanonicalExportDefaultTemplatesV1 {
    /// Projects defaults for a self-contained Export HIR graph. Imported-core
    /// calls are rejected because their process-local handles require the
    /// `OrdinaryHirOutput` selection sidecar.
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, DefaultTemplateProductionError> {
        Self::from_parts(export, None)
    }

    /// Projects defaults from an ordinary graph while resolving imported-core
    /// call handles against the exact selected-set world that admitted them.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
    ) -> Result<Self, DefaultTemplateProductionError> {
        let export = output.output().export.module();
        Self::from_parts(export, Some(output.imported_core()))
    }

    fn from_parts(
        export: &ExportHir,
        imported_core: Option<&SelectedImportedCoreSet<'_>>,
    ) -> Result<Self, DefaultTemplateProductionError> {
        let callables = CanonicalCallableInterfacesV1::from_export_hir(export)
            .map_err(DefaultTemplateProductionError::CallableInterfaces)?;
        let source_interfaces =
            CanonicalCallableSourceInterfacesV1::from_export_hir_with_callables(export, &callables)
                .map_err(DefaultTemplateProductionError::SourceInterfaces)?;
        Self::from_parts_with_interfaces(export, imported_core, &callables, &source_interfaces)
    }

    pub(in crate::production) fn from_parts_with_interfaces(
        export: &ExportHir,
        imported_core: Option<&SelectedImportedCoreSet<'_>>,
        callables: &CanonicalCallableInterfacesV1,
        source_interfaces: &CanonicalCallableSourceInterfacesV1,
    ) -> Result<Self, DefaultTemplateProductionError> {
        let owners = public_source_callable_owners(export, callables)
            .map_err(DefaultTemplateProductionError::SourceInterfaces)?;
        let entities = DefaultEntityProjector::new(export, imported_core);
        let mut templates = Vec::new();

        for owner in owners {
            let source = unique_source_interface(export, &owner)?;
            let canonical = source_interfaces.get(owner.declaration).ok_or(
                DefaultTemplateProductionError::MissingCanonicalSourceInterface(owner.declaration),
            )?;
            if source.parameters.len() != canonical.parameters().parameters().len() {
                return Err(DefaultTemplateProductionError::SourceParameterArity {
                    owner: owner.declaration,
                    hir: source.parameters.len(),
                    canonical: canonical.parameters().parameters().len(),
                });
            }
            for (index, parameter) in source.parameters.iter().enumerate() {
                let position = u32::try_from(index).map_err(|_| {
                    DefaultTemplateProductionError::TooManySourceParameters {
                        owner: owner.declaration,
                    }
                })?;
                let default = match parameter.calling {
                    ExportParameterCalling::Default { source, .. } => Some(source),
                    ExportParameterCalling::Vararg {
                        omission: crate::ExportVarargOmission::Default(source),
                        ..
                    } => Some(source),
                    ExportParameterCalling::Required { .. }
                    | ExportParameterCalling::Vararg {
                        omission: crate::ExportVarargOmission::EmptyArray,
                        ..
                    } => None,
                };
                if let Some(default) = default {
                    templates.push(envelope::project(
                        export, &entities, callables, &owner, position, default,
                    )?);
                }
            }
        }

        Self::try_new(templates).map_err(DefaultTemplateProductionError::Table)
    }
}

fn unique_source_interface<'a>(
    export: &'a ExportHir,
    owner: &SourceCallableOwner,
) -> Result<&'a ExportParameterInterface, DefaultTemplateProductionError> {
    let mut matches = export
        .source_parameter_interfaces
        .iter()
        .filter(|interface| interface.owner == owner.local);
    let Some(interface) = matches.next() else {
        return Err(DefaultTemplateProductionError::MissingSourceInterface(
            owner.declaration,
        ));
    };
    if matches.next().is_some() {
        return Err(DefaultTemplateProductionError::DuplicateSourceInterface(
            owner.declaration,
        ));
    }
    Ok(interface)
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

fn default_source_id(id: ExportDefaultSourceId) -> u32 {
    raw_index(id)
}

fn owner_index(owner: ExportParameterOwner) -> u32 {
    match owner {
        ExportParameterOwner::Function(id) => raw_index(id),
        ExportParameterOwner::StructConstructor(id) => raw_index(id),
        ExportParameterOwner::ClassConstructor(id) => raw_index(id),
        ExportParameterOwner::VariantConstructor(id) => id.local_index(),
    }
}
