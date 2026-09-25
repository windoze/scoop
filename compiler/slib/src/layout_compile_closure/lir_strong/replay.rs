use scoop_hir::PublicNominalShapeRequirementsV1;
use scoop_identity::{ConeCoordinate, ConeIdentity, SourceDeclarationKey};
use scoop_lir as lir;
use scoop_wire::{BudgetMeter, WireEncode, WirePath, encode_canonical_temporary_with_meter};

use super::{
    LirStrongProductionReplayedCrossConeLayoutSections, SharedLirStrongProductionError as Error,
};
use crate::layout_compile_decode::PreparedLayoutMirSemanticParts;

pub(super) fn replay(
    coordinate: ConeCoordinate,
    direct: &[ConeIdentity],
    target: lir::LirTargetProfile,
    strong: lir::InitializationAbiResolvedStrongProductionSectionV2,
    parts: PreparedLayoutMirSemanticParts<'_>,
    dependencies: &[&LirStrongProductionReplayedCrossConeLayoutSections<'_>],
) -> Result<lir::ReplayedStrongProductionSectionV2, Error> {
    let provider = parts.lir_foundation.producer();
    let external =
        strong.reconstruct_external_bridges_with_meter(provider, parts.identities, parts.meter)?;
    super::dependencies::validate_external_bridges(&external, dependencies, parts.meter)?;
    let (type_definitions, initialization_definitions) =
        super::dependencies::definitions(provider, dependencies, &external, parts.meter)?;
    let roots = PublicNominalShapeRequirementsV1::from_shared_surface(
        provider,
        parts.hir_core.direct_public_surface(),
        parts.hir_foundation.as_canonical(),
        parts.hir_interface.nominal_interfaces(),
        parts.hir_interface.callable_interfaces(),
        parts.meter,
    )?;
    let path = WirePath::root();
    let mut sources = Vec::new();
    parts
        .meter
        .try_reserve_collection_slots(&mut sources, roots.roots().len(), &path)?;
    for root in roots.roots() {
        parts.meter.charge_work(1, &path)?;
        let key = parts
            .identities
            .canonical_key::<_, SourceDeclarationKey>(root.source())?;
        charge_clone(key.as_ref(), parts.meter)?;
        sources.push(key.as_ref().clone());
    }
    let entry = match parts.mir_core.entry_bridge() {
        scoop_mir::EntryMirBridgeBranchV1::Library => lir::EntryProductionSourceV1::Library,
        scoop_mir::EntryMirBridgeBranchV1::Executable(bridge) => {
            charge_clone(bridge.source(), parts.meter)?;
            lir::EntryProductionSourceV1::executable(bridge.source().clone())
        }
    };
    let initialization = match strong.initialization_cycle_abi() {
        Some(abi) => {
            charge_clone(abi, parts.meter)?;
            Some(Box::new(abi.clone()))
        }
        None => None,
    };
    Ok(strong.replay(
        coordinate,
        direct,
        target,
        parts.lir_foundation,
        external,
        entry,
        &sources,
        initialization,
        &type_definitions,
        &initialization_definitions,
        parts.meter,
    )?)
}

fn charge_clone(value: &impl WireEncode, meter: &mut BudgetMeter) -> Result<(), Error> {
    let path = WirePath::root();
    let bytes = encode_canonical_temporary_with_meter(value, meter, &path)?;
    // Accounts for dynamic key/signature elements and their owned text before
    // the existing typed constructors retain a copy.
    meter.charge_collection_slots(bytes.len() as u64, &path)?;
    meter.charge_work(bytes.len() as u64, &path)?;
    Ok(())
}
