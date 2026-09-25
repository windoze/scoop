//! Dispatch export from the sealed HIR choices and actual MIR tables.

use scoop_hir as hir;
use scoop_identity::{
    CallableOwner, DispatchDeclarationOwner, DispatchSlotKey, ExactCallableSignature,
    PersistentDispatchSlotId, PersistentExactTypeId, StrongCallableDefinitionOwner,
};
use scoop_mir as mir;
use scoop_wire::{WireError, WirePath};
use std::collections::BTreeMap;

mod context;
mod entries;
mod physical;
use context::*;

pub fn lower_dispatch_schemas(
    source: &hir::CrossConeTypeSemanticsProductionV1,
    input: &mir::SingleConeStrongMirInput,
    local_types: &mir::CanonicalParamFreeMirTypeExportsV1,
    authority: mir::MirDispatchSchemaAuthority<'_>,
    dependencies: &[&mir::CanonicalMirDispatchSchemasV1],
) -> Result<mir::CanonicalMirDispatchSchemasV1, SourceMirDispatchProductionError> {
    let context = Context::new(source, input, authority)?;
    let mut records = reserve(
        source
            .inheritance_inventory()
            .records()
            .len()
            .saturating_mul(2),
    )?;
    for source in source.inheritance_inventory().records() {
        let record = local_types
            .get(source.owner())
            .ok_or(Error::MissingType(source.owner()))?;
        records.push(context.record(source, source.owner())?);
        if let mir::MirTypeRepresentationV1::Object { backing } = record.representation() {
            if local_types.get(*backing).is_none() {
                return Err(Error::MissingType(*backing));
            }
            records.push(context.record(source, *backing)?);
        }
    }

    if let Some(builtin) = input.module().meta.source_exact_types.get(&mir::Type::Any) {
        let exact = builtin.identity_record().id();

        if local_types.get(exact).is_some() {
            scoop_wire::allocation::try_reserve(&mut records, 1, &WirePath::root())?;
            records.push(mir::ParamFreeMirDispatchSchemaV1::try_new(
                authority,
                exact,
                mir::MirClassVtableSchemaV1::ClassVtable(vec![]),
                vec![],
            )?);
        }
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
    Schema(mir::MirDispatchSchemaError),
    Identity(scoop_identity::IdentityReferenceError),
    MissingType(PersistentExactTypeId),
    MissingPhysicalType(PersistentExactTypeId),
    MissingCallable(StrongCallableDefinitionOwner),
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
