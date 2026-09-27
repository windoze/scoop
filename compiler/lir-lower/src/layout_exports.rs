//! Complete physical export assembly from one sealed MIR/LIR production pair.

use scoop_identity::{
    ConeCoordinate, ConeIdentity, ExactTypeDiagnosticCatalog, PersistentDispatchTableId,
    PersistentExactTypeId, StrongCallableDefinitionOwner, ValidatedIdentityGraph,
};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{WireError, WirePath};

mod callables;
mod descriptors;
mod dispatch;
mod error;
mod lookup;
mod source;
use LayoutAbiExportLoweringError as Error;
pub use error::LayoutAbiExportLoweringError;
pub use source::{LayoutAbiDependencyLoweringError, lower_layout_abi_dependencies};

#[derive(Clone, Copy)]
pub struct LayoutAbiExportInputV1<'a> {
    pub mir: &'a mir::ConeMirInput,
    pub lir: &'a lir::ConeLirOutput,
    pub bridge: &'a mir::MirTypeBridgeExportConstituentsV1,
    pub ordinary: &'a lir::CrossConeLirBridgeSectionV1,
    pub registration: &'a lir::StrongProductionSectionV2,
    pub identities: &'a ValidatedIdentityGraph,
    pub coordinates: &'a [ConeCoordinate],
}

#[derive(Clone, Copy, Default)]
pub struct LayoutAbiExportDependenciesV1<'a> {
    pub layouts: &'a [&'a lir::CanonicalExactLayoutExportsV1],
    pub callables: &'a [&'a lir::CanonicalExactCallableAbiExportsV1],
    pub direct_callables: &'a [&'a lir::CrossConeLirBridgeSectionV1],
}

/// Replays all five physical export inventories. The enclosing section still
/// closes source contracts, selected uses and terminal dependency sections.
pub fn lower_layout_abi_exports(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
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
    lookup::validate_dependencies(provider, target, dependencies)?;
    let layouts = crate::lower_exact_layout_exports(
        input.mir,
        input.lir,
        input.bridge.types(),
        input.identities,
        dependencies.layouts,
    )?;
    let lookup = lookup::Layouts {
        local: &layouts,
        dependencies: dependencies.layouts,
    };
    let callables = callables::lower(input)?;
    let diagnostics = ExactTypeDiagnosticCatalog::try_new(input.identities, input.coordinates)?;
    let descriptors = descriptors::lower(input, &layouts, &diagnostics)?;
    let dispatch = dispatch::lower(input, &lookup, &callables, dependencies)?;
    let roots = input.lir.shape_support().roots();
    let mut sources = reserve(roots.len())?;
    for root in roots {
        sources.push(root.declaration().clone());
    }
    let shapes = lir::CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &sources,
        &layouts,
        &descriptors,
        input.lir.foundation(),
    )?;
    Ok(lir::LayoutAbiExportConstituentsV1::try_new(
        layouts,
        descriptors,
        dispatch,
        callables,
        shapes,
        input.ordinary.clone(),
    )?)
}

fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let path = WirePath::root();

    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, count, &path)?;
    Ok(values)
}
