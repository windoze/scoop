//! Complete local export assembly from one sealed HIR/MIR production pair.

use scoop_hir as hir;
use scoop_identity::{PersistentInitializationUnitId, ValidatedIdentityGraph};
use scoop_mir as mir;
use scoop_wire::{WireError, WirePath};

mod callables;
mod error;
mod initialization;
mod inputs;
mod source;
use MirTypeBridgeExportProductionError as Error;
pub use error::MirTypeBridgeExportProductionError;
pub use inputs::{MirTypeBridgeDependencyTablesV1, MirTypeBridgeExportInputV1};
pub use source::{
    MirTypeBridgeUseLoweringError, lower_type_bridge_dependencies,
    lower_type_bridge_initialization_units,
};

/// Produces all six local export tables. Dependency tables remain borrowed;
/// the enclosing section joins their references with the actual dependency uses.
pub fn lower_type_bridge_exports(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: MirTypeBridgeDependencyTablesV1<'_>,
) -> Result<mir::MirTypeBridgeExportConstituentsV1, Error> {
    input.validate()?;
    let initialization_uses = initialization::project(input)?;
    let types = crate::lower_type_exports(
        input.hir.output().local.module(),
        input.source,
        input.mir,
        input.identities,
    )
    .map_err(Error::Types)?;
    let tables = with_local(&types, dependencies.types)?;
    let type_index = mir::MirTypeBridgeTypeIndexV1::try_new(&tables).map_err(Error::Lookup)?;
    let source_callables = crate::lower_source_callable_bindings(
        input.hir,
        input.public,
        input.source,
        input.mir,
        input.identities,
        &type_index,
        input.ordinary.exports(),
    )
    .map_err(Error::SourceCallables)?;
    let constructors = crate::lower_constructor_bindings(
        input.hir,
        input.public,
        input.mir,
        input.identities,
        &type_index,
    )
    .map_err(Error::Constructors)?;
    let (object_callables, objects) = mir::MirObjectValueProductionV1::from_strong_input(
        input.mir,
        &types,
        input.identities,
        &type_index,
    )
    .map_err(Error::Objects)?
    .into_parts();
    let source_tables = with_local(&source_callables, dependencies.callables)?;
    let direct_tables = with_local(input.ordinary, dependencies.direct_callables)?;
    let source_index = mir::MirTypeBridgeCallableIndexV1::try_new(&source_tables, &direct_tables)
        .map_err(Error::Lookup)?;
    let boxing = mir::CanonicalMirCallableBindingsV1::from_boxing_adjusts(
        input.mir,
        &types,
        input.identities,
        &type_index,
        &source_index,
    )
    .map_err(Error::Boxing)?;
    let equality = crate::lower_derived_equality_bindings(
        input.hir,
        input.mir,
        &types,
        input.identities,
        &type_index,
    )
    .map_err(Error::Equality)?;
    let callables = callables::combine([
        source_callables,
        constructors,
        object_callables,
        boxing,
        equality,
    ])?;
    let callable_tables = with_local(&callables, dependencies.callables)?;
    let callable_index =
        mir::MirTypeBridgeCallableIndexV1::try_new(&callable_tables, &direct_tables)
            .map_err(Error::Lookup)?;
    let dispatch = crate::lower_dispatch_schemas(
        input.hir.output().local.module(),
        input.source,
        input.mir,
        &types,
        mir::MirDispatchSchemaAuthority {
            identities: input.identities,
            types: &type_index,
            callables: &callable_index,
        },
        dependencies.dispatch,
    )
    .map_err(Error::Dispatch)?;
    let shapes =
        mir::CanonicalMirShapeSupportsV1::from_strong_input(input.mir, input.identities, &types)
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

fn with_local<'a, T>(local: &'a T, dependencies: &[&'a T]) -> Result<Vec<&'a T>, Error> {
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(Error::CountOverflow)?;
    let mut tables = reserve(count)?;
    tables.push(local);
    tables.extend_from_slice(dependencies);
    Ok(tables)
}

fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let path = WirePath::root();

    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, count, &path)?;
    Ok(values)
}
