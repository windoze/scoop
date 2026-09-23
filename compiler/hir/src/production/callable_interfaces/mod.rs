//! Projection of the complete public HIR callable surface.

use super::signatures::HirInterfaceSignatureProjector;
use crate::{CanonicalCallableInterfacesV1, ExportHir};

mod access;
mod accessors;
mod constructors;
mod effects;
pub(in crate::production) use effects::accessor as source_accessor_effects;
pub(in crate::production) use effects::function as source_function_effects;
pub(in crate::production) use effects::source_constructor as source_constructor_effects;
mod errors;
mod functions;
mod parameters;
pub(in crate::production) use parameters::project_source as source_parameter_shapes;
mod slots;
mod support;
mod variants;

pub use errors::{
    CallableAccessProjectionError, CallableEffectProjectionError, CallableInterfaceBuildError,
    CallableProjectionError, CallableProjectionSubject, SourceParameterProjectionError,
};

struct CallableProjection<'a> {
    export: &'a ExportHir,
    signatures: HirInterfaceSignatureProjector<'a>,
    properties: &'a crate::CanonicalPropertyInterfacesV1,
}

impl CanonicalCallableInterfacesV1 {
    /// Projects public callables and the source callables required by shared
    /// nominal declarations. Generated constructor adapters and accessor
    /// implementation functions remain outside this declaration interface.
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, CallableInterfaceBuildError> {
        Self::from_export_hir_with_budget(
            export,
            &mut scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default()),
        )
    }

    pub fn from_export_hir_with_budget(
        export: &ExportHir,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<Self, CallableInterfaceBuildError> {
        let properties = crate::CanonicalPropertyInterfacesV1::from_export_hir(export)
            .map_err(CallableInterfaceBuildError::PropertyInterfaces)?;
        let nominals =
            crate::CanonicalNominalInterfacesV1::from_export_hir_with_budget(export, meter)
                .map_err(CallableInterfaceBuildError::Nominals)?;
        Self::from_export_hir_with_nominals(export, &properties, &nominals, meter)
    }

    pub(in crate::production) fn from_export_hir_with_nominals(
        export: &ExportHir,
        properties: &crate::CanonicalPropertyInterfacesV1,
        nominals: &crate::CanonicalNominalInterfacesV1,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<Self, CallableInterfaceBuildError> {
        let projection = CallableProjection {
            export,
            signatures: HirInterfaceSignatureProjector::new(export),
            properties,
        };
        let mut records = Vec::new();
        functions::project_all(&projection, &mut records)?;
        constructors::project_all(&projection, &mut records)?;
        variants::project_all(&projection, &mut records)?;
        accessors::project_all(&projection, &mut records)?;
        let support = support::project(&projection, nominals, &records, meter)?;
        let callables =
            Self::with_support(records, support).map_err(CallableInterfaceBuildError::Table)?;
        callables
            .validate_declaration_inventory(nominals, meter)
            .map_err(CallableInterfaceBuildError::Inventory)?;
        projection
            .properties
            .validate_accessor_closure(&callables)
            .map_err(CallableInterfaceBuildError::AccessorClosure)?;
        Ok(callables)
    }
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
