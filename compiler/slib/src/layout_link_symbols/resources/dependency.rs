use super::*;
use std::collections::BTreeMap;

impl SymbolCosts {
    pub(in super::super) fn dependency(
        &self,
        objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
        target: lir::LirTargetProfile,
        strong: &lir::ReplayedStrongProductionSectionV2,
        ordinary: &lir::CrossConeLirBridgeSectionV1,
        dependencies: &[CanonicalDefinedLinkSymbolOwnerSetV1],
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        objects.charge_strong_closure_copy(meter)?;
        copy_plan(strong.external_bridges(), meter)?;
        copy_plan(ordinary, meter)?;
        table::<(scoop_identity::ConeIdentity, usize)>(dependencies.len() as u64, meter)?;
        let mut owner_count = 0;
        for owners in dependencies {
            owner_count = owner_count.max(owners.owners().len() as u64);
            copy_owners(owners, meter)?;
        }
        let symbols = (strong.external_bridges().bridges().len() as u64)
            .saturating_mul(2)
            .saturating_add(ordinary.selected().len() as u64);
        table::<(Vec<u8>, usize)>(symbols.saturating_mul(3), meter)?;
        for bridge in strong.external_bridges().bridges() {
            let request = match bridge {
                lir::StrongExternalLirBridgeV1::Callable(bridge) => {
                    bridge.bridge().expected_symbol()
                }
                lir::StrongExternalLirBridgeV1::TypeDescriptor(bridge) => bridge.expected_symbol(),
            };
            normalized(request, symbols, owner_count, 2, meter)?;
        }
        for selected in ordinary.selected() {
            normalized(
                selected.bridge().expected_symbol(),
                symbols,
                owner_count,
                1,
                meter,
            )?;
        }
        meter.charge_work(
            self.bindings
                .saturating_mul(self.longest_name + 1)
                .saturating_mul(log(symbols)),
            &WirePath::root(),
        )?;
        self.uses::<DependencyStrongRequirementUseV1>(4, meter)?;
        callable_copies(objects, target, strong.external_bridges(), meter)
    }
}

fn normalized(
    request: scoop_identity::PersistentSymbolRequest,
    symbols: u64,
    owners: u64,
    copies: u64,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let size = scoop_identity::MangledSymbol::byte_length_for_key(&request.key()) as u64 + 1;
    meter.charge_owned_bytes(
        size.saturating_mul(copies).saturating_mul(3),
        &WirePath::root(),
    )?;
    meter.charge_work(
        size.saturating_mul(copies)
            .saturating_mul(log(symbols) + log(owners) + 1),
        &WirePath::root(),
    )
}

fn callable_copies(
    objects: &ReplayedLayoutLinkObjectContentsV1<'_>,
    target: lir::LirTargetProfile,
    bridges: &lir::StrongExternalLirBridgeSurfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    let count = bridges.bridges().len() as u64;
    table::<(Vec<u8>, &lir::StrongExternalLirBridgeV1)>(count, meter)?;
    let mut callables = BTreeMap::new();
    for bridge in bridges.bridges() {
        let lir::StrongExternalLirBridgeV1::Callable(callable) = bridge else {
            continue;
        };
        let name = target
            .contract()
            .native_symbol_normalization()
            .compiler_generated_object_symbol(callable.bridge().expected_symbol().symbol().as_str())
            .into_bytes();
        meter.charge_work((name.len() as u64 + 1).saturating_mul(log(count)), &path)?;
        callables.insert(name, bridge);
    }
    // The shared classifier clones the full ABI for every matched physical
    // use, including repeated calls to the same external callable.
    for binding in objects
        .patch_sites()
        .builtins()
        .strong_relocations()
        .bindings()
    {
        meter.charge_work(
            (binding.symbol().len() as u64 + 1).saturating_mul(log(count)),
            &path,
        )?;
        if let Some(bridge) = callables.get(binding.symbol()) {
            copy_plan(*bridge, meter)?;
        }
    }
    Ok(())
}
