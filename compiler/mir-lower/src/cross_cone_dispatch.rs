//! Dispatch export from the sealed HIR choices and actual MIR tables.

use scoop_hir as hir;
use scoop_identity::{
    CallableDefinitionOwner, CallableOwner, DispatchDeclarationOwner, DispatchSlotKey,
    ExactCallableSignature, PersistentDispatchSlotId, PersistentExactTypeId,
    StrongCallableDefinitionOwner,
};
use scoop_mir as mir;
use scoop_wire::{WireError, WirePath};
use std::collections::BTreeMap;

mod applications;
mod context;
mod entries;
mod physical;
mod slots;
use context::*;

pub fn lower_dispatch_schemas(
    local: &hir::LocalConcreteHir,
    source: &hir::CrossConeTypeSemanticsSectionV1,
    input: &mir::ConeMirInput,
    local_types: &mir::CanonicalParamFreeMirTypeExportsV1,
    authority: mir::MirDispatchSchemaAuthority<'_>,
    dependencies: &[&mir::CanonicalMirDispatchSchemasV1],
) -> Result<mir::CanonicalMirDispatchSchemasV1, SourceMirDispatchProductionError> {
    let context = Context::new(source, input, authority)?;
    let mut records = reserve(source.inheritance().records().len().saturating_mul(2))?;
    for source in source.inheritance().records() {
        let record = local_types
            .get(source.owner())
            .ok_or(Error::MissingType(source.owner()))?;
        records.push(context.record(local, source, source.owner())?);
        if let mir::MirTypeRepresentationV1::Object { backing } = record.representation() {
            if local_types.get(*backing).is_none() {
                return Err(Error::MissingType(*backing));
            }
            records.push(context.record(local, source, *backing)?);
        }
    }

    applications::append(local, &context, local_types, &mut records)?;

    for ty in local_types.records() {
        let mir::MirTypeOriginV1::GeneratedNominal {
            role: scoop_identity::GeneratedNominalKey::TaskContext(storage),
            ..
        } = ty.origin()
        else {
            continue;
        };
        let slots = match storage.role {
            mir::ContextStorageRole::Task
            | mir::ContextStorageRole::Node
            | mir::ContextStorageRole::Binding => mir::MirDispatchSlotsV1::ClassVtable(Vec::new()),
            mir::ContextStorageRole::Mark | mir::ContextStorageRole::SwitchGuard => {
                mir::MirDispatchSlotsV1::NoClassVtable
            }
        };
        records.push(mir::ParamFreeMirDispatchSchemaV1::try_new(
            authority,
            ty.exact(),
            slots,
            Vec::new(),
        )?);
    }

    Ok(
        mir::CanonicalMirDispatchSchemasV1::try_new_with_dependencies(
            authority,
            records,
            dependencies,
        )?,
    )
}

type Error = SourceMirDispatchProductionError;

#[derive(Debug)]
pub enum SourceMirDispatchProductionError {
    Resource(WireError),
    RuntimeSlot(mir::RuntimeFn),
    Application(PersistentExactTypeId),
    Callable(mir::MirCallableBridgeError),
    Schema(mir::MirDispatchSchemaError),
    Identity(scoop_identity::IdentityReferenceError),
    MissingType(PersistentExactTypeId),
    MissingPhysicalType(PersistentExactTypeId),
    MissingCallable(CallableDefinitionOwner),
    MissingStrongRoot(mir::FunctionId),
    InvalidTarget(CallableOwner),
    MissingSelection {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    },
    TableMismatch {
        owner: PersistentExactTypeId,
        role: hir::InheritanceSlotSchemaRoleV1,
    },
    TargetMismatch {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    },
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<mir::MirDispatchSchemaError> for Error {
    fn from(error: mir::MirDispatchSchemaError) -> Self {
        Self::Schema(error)
    }
}
impl From<scoop_identity::IdentityReferenceError> for Error {
    fn from(error: scoop_identity::IdentityReferenceError) -> Self {
        Self::Identity(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot produce source MIR dispatch schemas: {self:?}")
    }
}
impl std::error::Error for Error {}

fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, count, &WirePath::root())?;
    Ok(values)
}

impl From<mir::MirCallableBridgeError> for SourceMirDispatchProductionError {
    fn from(error: mir::MirCallableBridgeError) -> Self {
        Self::Callable(error)
    }
}
