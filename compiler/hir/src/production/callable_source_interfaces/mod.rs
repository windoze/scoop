//! Projection of source-level argument protocols for all shared source declarations.

use scoop_identity::CallableTemplateOrigin;

use super::signatures::HirInterfaceSignatureProjector;
use crate::{CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1, ExportHir};

mod errors;
mod owners;
mod parameters;

pub use errors::{
    CallableSourceInterfaceProductionError, CallableSourceParameterProjectionError,
    SourceCallableOwnerProjectionError,
};

pub(super) struct CallableSourceProjection<'a> {
    export: &'a ExportHir,
    callables: &'a CanonicalCallableInterfacesV1,
    signatures: HirInterfaceSignatureProjector<'a>,
}

pub(super) struct SourceCallableOwner {
    pub(super) subject: crate::CallableProjectionSubject,
    pub(super) local: crate::ExportParameterOwner,
    pub(super) declaration: CallableTemplateOrigin,
    pub(super) binders: Vec<crate::HirSignatureBinder>,
}

pub(super) fn shared_source_callable_owners<'a>(
    export: &'a ExportHir,
    callables: &'a CanonicalCallableInterfacesV1,
) -> Result<Vec<SourceCallableOwner>, CallableSourceInterfaceProductionError> {
    let projection = CallableSourceProjection {
        export,
        callables,
        signatures: HirInterfaceSignatureProjector::new(export),
    };
    owners::collect(&projection)
}

impl CanonicalCallableSourceInterfacesV1 {
    /// Projects exactly one source-call interface for every shared source
    /// callable. Property accessors are intentionally absent because callers
    /// never apply named/default/vararg source argument rules to accessors.
    pub fn from_export_hir(
        export: &ExportHir,
    ) -> Result<Self, CallableSourceInterfaceProductionError> {
        let callables = CanonicalCallableInterfacesV1::from_export_hir(export)
            .map_err(CallableSourceInterfaceProductionError::CallableInterfaces)?;
        Self::from_export_hir_with_callables(export, &callables)
    }

    pub(super) fn from_export_hir_with_callables(
        export: &ExportHir,
        callables: &CanonicalCallableInterfacesV1,
    ) -> Result<Self, CallableSourceInterfaceProductionError> {
        let projection = CallableSourceProjection {
            export,
            callables,
            signatures: HirInterfaceSignatureProjector::new(export),
        };
        let owners = owners::collect(&projection)?;
        let mut records = Vec::with_capacity(owners.len());
        for owner in owners {
            records.push(parameters::project(&projection, owner)?);
        }
        let interfaces =
            Self::try_new(records).map_err(CallableSourceInterfaceProductionError::Table)?;
        validate_closure(callables, &interfaces)?;
        Ok(interfaces)
    }
}

fn validate_closure(
    callables: &CanonicalCallableInterfacesV1,
    interfaces: &CanonicalCallableSourceInterfacesV1,
) -> Result<(), CallableSourceInterfaceProductionError> {
    for callable in callables.all_declarations() {
        let declaration = callable.declaration();
        if matches!(declaration, CallableTemplateOrigin::Accessor(_)) {
            if interfaces.get(declaration).is_some() {
                return Err(
                    CallableSourceInterfaceProductionError::PropertyAccessorInterface(declaration),
                );
            }
        } else if interfaces.get(declaration).is_none() {
            return Err(CallableSourceInterfaceProductionError::MissingInterface(
                declaration,
            ));
        }
    }
    Ok(())
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
