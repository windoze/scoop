//! Bind shared-reader physical contracts to actual provider definitions.

use super::*;

pub fn verify_replayed_layout_strong_owners_v1(
    layout: &scoop_lir::PhysicalImportsReplayedLayoutAbiSectionV1<'_>,
    current: &CanonicalDefinedLinkSymbolOwnerSetV1,
    dependencies: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    let consumer = layout.exports().provider();
    if current.producer() != consumer {
        return Err(
            CrossConeLayoutTerminalValidationError::ArtifactIdentityMismatch {
                foundation: consumer,
                section: consumer,
                defined_symbols: current.producer(),
            },
        );
    }
    let path = WirePath::root();
    let count = dependencies.len().saturating_add(1) as u64;
    meter.check_table_entries(count, &path)?;
    meter.charge_collection_slots(count, &path)?;
    meter.charge_owned_bytes(
        count.saturating_mul(std::mem::size_of::<(
            ConeIdentity,
            (usize, &CanonicalDefinedLinkSymbolOwnerSetV1),
        )>() as u64),
        &path,
    )?;
    meter.charge_work(
        count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
        &path,
    )?;
    let mut providers = BTreeMap::new();
    for (index, artifact) in std::iter::once(current).chain(dependencies).enumerate() {
        if let Some(previous) = providers.insert(artifact.producer(), (index, artifact)) {
            return Err(CrossConeLayoutTerminalValidationError::DuplicateProvider {
                provider: artifact.producer(),
                first: previous.0,
                second: index,
            });
        }
    }
    for import in layout.physical_imports().records() {
        meter.charge_work(u64::from(count.max(1).ilog2()) + 1, &path)?;
        let (_, terminal) = providers.get(&import.provider()).ok_or(
            CrossConeLayoutTerminalValidationError::MissingProvider {
                consumer,
                provider: import.provider(),
            },
        )?;
        owner::validate_sets(
            consumer,
            layout.exports().target_profile(),
            import,
            terminal,
            std::iter::once(current).chain(dependencies),
            meter,
        )?;
    }
    Ok(())
}
