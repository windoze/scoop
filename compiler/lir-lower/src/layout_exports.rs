//! Complete physical export assembly from one sealed MIR/LIR production pair.

use scoop_identity::{
    ConeCoordinate, ConeIdentity, ExactTypeDiagnosticCatalog, PersistentDispatchTableId,
    PersistentExactTypeId, StrongCallableDefinitionOwner, ValidatedIdentityGraph,
};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WireError, WirePath};

mod callables;
mod descriptors;
mod dispatch;
mod error;
mod lookup;
use LayoutAbiExportLoweringError as Error;
pub use error::LayoutAbiExportLoweringError;

#[derive(Clone, Copy)]
pub struct LayoutAbiExportInputV1<'a> {
    pub mir: &'a mir::SingleConeStrongMirInput,
    pub lir: &'a lir::SingleConeStrongLirOutput,
    pub bridge: &'a mir::MirTypeBridgeExportConstituentsV1,
    pub registration: &'a lir::PendingStrongProductionSectionV2,
    pub identities: &'a ValidatedIdentityGraph,
    pub coordinates: &'a [ConeCoordinate],
}

#[derive(Clone, Copy, Default)]
pub struct LayoutAbiExportDependenciesV1<'a> {
    pub layouts: &'a [&'a lir::CanonicalExactLayoutExportsV1],
    pub callables: &'a [&'a lir::CanonicalExactCallableAbiExportsV1],
}

/// Replays all five physical export inventories. The enclosing section still
/// closes source contracts, selected uses and terminal dependency sections.
pub fn lower_layout_abi_exports(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<lir::LayoutAbiExportConstituentsV1, Error> {
    let provider = input.lir.foundation().producer();
    let target = input.lir.module().meta.target_profile;
    let registrations = input.registration.registration_production().types();
    if input.mir.module().cone != provider || registrations.producer() != provider {
        return Err(Error::Provider);
    }
    if registrations.target() != &target.wire_id() {
        return Err(Error::Target);
    }
    lookup::validate_dependencies(provider, target, dependencies, meter)?;
    let layouts = crate::lower_exact_layout_exports(
        input.mir,
        input.lir,
        input.bridge.types(),
        input.identities,
        dependencies.layouts,
        meter,
    )?;
    let lookup = lookup::Layouts {
        local: &layouts,
        dependencies: dependencies.layouts,
    };
    let callables = callables::lower(input, &lookup, meter)?;
    let diagnostics =
        ExactTypeDiagnosticCatalog::try_new(input.identities, input.coordinates, meter)?;
    let descriptors = descriptors::lower(input, &layouts, &diagnostics, meter)?;
    let dispatch = dispatch::lower(input, &lookup, &callables, dependencies.callables, meter)?;
    let roots = input.lir.shape_support().roots();
    let mut sources = reserve(roots.len(), meter)?;
    for root in roots {
        meter.charge_owned_bytes(
            scoop_wire::encoded_length(root.declaration())?,
            &WirePath::root(),
        )?;
        sources.push(root.declaration().clone());
    }
    let shapes = lir::CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &sources,
        &layouts,
        &descriptors,
        input.lir.foundation(),
        meter,
    )?;
    Ok(lir::LayoutAbiExportConstituentsV1::try_new(
        layouts,
        descriptors,
        dispatch,
        callables,
        shapes,
    )?)
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

fn search(count: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(u64::from(count.max(1).ilog2()) + 1, &WirePath::root())?)
}
