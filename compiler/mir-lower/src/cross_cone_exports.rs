//! Complete local export assembly from one sealed HIR/MIR production pair.

use scoop_hir as hir;
use scoop_identity::{
    PersistentInitializationUnitId, StrongCallableDefinitionOwner, ValidatedIdentityGraph,
};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WireError, WirePath};

mod callables;
mod error;
mod inputs;
use MirTypeBridgeExportProductionError as Error;
pub use error::MirTypeBridgeExportProductionError;
pub use inputs::{MirTypeBridgeDependencyTablesV1, MirTypeBridgeExportInputV1};

/// Produces all six local export tables. Dependency tables remain borrowed;
/// the enclosing section still owns source and committed-use closure replay.
pub fn lower_type_bridge_exports(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: MirTypeBridgeDependencyTablesV1<'_>,
    initialization_uses: mir::CanonicalMirExternalInitializationUsesV1,
    meter: &mut BudgetMeter,
) -> Result<mir::MirTypeBridgeExportConstituentsV1, Error> {
    input.validate(&initialization_uses, meter)?;
    let types = crate::lower_type_exports(input.source, input.mir, input.identities, meter)
        .map_err(Error::Types)?;
    let tables = with_local(&types, dependencies.types, meter)?;
    let type_index =
        mir::MirTypeBridgeTypeIndexV1::try_new(&tables, meter).map_err(Error::Lookup)?;
    let source_callables = crate::lower_source_callable_bindings(
        input.hir,
        input.public,
        input.source,
        input.mir,
        input.identities,
        &type_index,
        meter,
    )
    .map_err(Error::SourceCallables)?;
    let constructors = crate::lower_constructor_bindings(
        input.hir,
        input.source,
        input.mir,
        input.identities,
        &type_index,
        meter,
    )
    .map_err(Error::Constructors)?;
    let (object_callables, objects) = mir::MirObjectValueProductionV1::from_strong_input(
        input.mir,
        &types,
        input.identities,
        &type_index,
        meter,
    )
    .map_err(Error::Objects)?
    .into_parts();
    let source_tables = with_local(&source_callables, dependencies.callables, meter)?;
    let source_index =
        mir::MirTypeBridgeCallableIndexV1::try_new(&source_tables, meter).map_err(Error::Lookup)?;
    let boxing = mir::CanonicalMirCallableBindingsV1::from_boxing_adjusts(
        input.mir,
        &types,
        input.identities,
        &type_index,
        &source_index,
        meter,
    )
    .map_err(Error::Boxing)?;
    let equality = crate::lower_derived_equality_bindings(
        input.hir,
        input.mir,
        &types,
        input.identities,
        &type_index,
        meter,
    )
    .map_err(Error::Equality)?;
    let callables = callables::combine(
        [
            source_callables,
            constructors,
            object_callables,
            boxing,
            equality,
        ],
        input.ordinary,
        meter,
    )?;
    let callable_tables = with_local(&callables, dependencies.callables, meter)?;
    let callable_index = mir::MirTypeBridgeCallableIndexV1::try_new(&callable_tables, meter)
        .map_err(Error::Lookup)?;
    let dispatch = crate::lower_dispatch_schemas(
        input.source,
        input.mir,
        &types,
        mir::MirDispatchSchemaAuthority {
            identities: input.identities,
            types: &type_index,
            callables: &callable_index,
        },
        dependencies.dispatch,
        meter,
    )
    .map_err(Error::Dispatch)?;
    let shapes = mir::CanonicalMirShapeSupportsV1::from_strong_input(
        input.mir,
        input.identities,
        &types,
        meter,
    )
    .map_err(Error::Shapes)?;
    Ok(mir::MirTypeBridgeExportConstituentsV1::new(
        types,
        callables,
        dispatch,
        objects,
        shapes,
        initialization_uses,
    ))
}

fn with_local<'a, T>(
    local: &'a T,
    dependencies: &[&'a T],
    meter: &mut BudgetMeter,
) -> Result<Vec<&'a T>, Error> {
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(Error::CountOverflow)?;
    let mut tables = reserve(count, meter)?;
    tables.push(local);
    tables.extend_from_slice(dependencies);
    Ok(tables)
}

fn reserve<T>(count: usize, meter: &mut BudgetMeter) -> Result<Vec<T>, Error> {
    let path = WirePath::root();
    meter.check_table_entries(count as u64, &path)?;
    meter.charge_owned_bytes(
        (count as u64).saturating_mul(std::mem::size_of::<T>() as u64),
        &path,
    )?;
    let mut values = Vec::new();
    meter.try_reserve_collection_slots(&mut values, count, &path)?;
    Ok(values)
}
