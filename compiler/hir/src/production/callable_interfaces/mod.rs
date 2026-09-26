//! Projection of the complete public HIR callable surface.

use super::nominal_interfaces::SharedSourceRoots;
use super::signatures::HirInterfaceSignatureProjector;
use crate::{CanonicalCallableInterfacesV1, ExportHir};

mod access;
mod accessors;
mod constructors;
mod effects;
pub(in crate::production) use effects::function as source_function_effects;
mod errors;
mod functions;
mod nominal_declarations;
mod parameters;
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
        let roots = SharedSourceRoots::from_export_hir(export)
            .map_err(CallableInterfaceBuildError::Nominals)?;
        let nominals =
            crate::CanonicalNominalInterfacesV1::from_export_hir_with_source_roots(export, &roots)
                .map_err(CallableInterfaceBuildError::Nominals)?;
        let properties = crate::CanonicalPropertyInterfacesV1::from_export_hir_with_nominals(
            export, &nominals, &roots,
        )
        .map_err(CallableInterfaceBuildError::PropertyInterfaces)?;
        Self::from_export_hir_with_nominals(export, &properties, &nominals, &roots)
    }

    pub(in crate::production) fn from_export_hir_with_nominals(
        export: &ExportHir,
        properties: &crate::CanonicalPropertyInterfacesV1,
        nominals: &crate::CanonicalNominalInterfacesV1,
        roots: &SharedSourceRoots,
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
        let support =
            support::project(&projection, nominals, &records, &roots.top_level_callables)?;
        let callables =
            Self::with_support(records, support).map_err(CallableInterfaceBuildError::Table)?;
        callables
            .validate_member_declaration_inventory(nominals, properties)
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
