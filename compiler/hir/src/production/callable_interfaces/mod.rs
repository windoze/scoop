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
    /// Projects every source callable in the current Cone's foreign-public
    /// surface. Generated constructor adapters and accessor implementation
    /// functions deliberately remain outside this declaration interface.
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, CallableInterfaceBuildError> {
        let properties = crate::CanonicalPropertyInterfacesV1::from_export_hir(export)
            .map_err(CallableInterfaceBuildError::PropertyInterfaces)?;
        Self::from_export_hir_with_properties(export, &properties)
    }

    pub(in crate::production) fn from_export_hir_with_properties(
        export: &ExportHir,
        properties: &crate::CanonicalPropertyInterfacesV1,
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
        let callables = Self::try_new(records).map_err(CallableInterfaceBuildError::Table)?;
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
