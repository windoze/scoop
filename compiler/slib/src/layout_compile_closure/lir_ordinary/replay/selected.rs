use std::collections::BTreeMap;

use scoop_identity::{ConeIdentity, PersistentExactTypeId, ScoopAbiArgument};

use super::*;

pub(super) fn replay(
    mir: &mir::CrossConeMirBridgeSectionV1,
    dependencies: SharedOrdinaryLirBridgeDependenciesV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<Vec<lir::SelectedDependencyLirCallableV1>, Error> {
    let path = WirePath::root();
    if dependencies.metadata.len() != dependencies.callables.len() {
        return Err(Error::DependencySources);
    }
    let mut providers = BTreeMap::new();
    for table in dependencies.callables {
        let provider = table.artifact();
        meter.charge_work(
            u64::from(providers.len().max(1).ilog2())
                + dependencies.layouts.len() as u64
                + dependencies.metadata.len() as u64
                + 1,
            &path,
        )?;
        if provider == mir.artifact()
            || providers.contains_key(&provider)
            || !dependencies
                .layouts
                .iter()
                .any(|layouts| layouts.provider() == provider)
            || !dependencies
                .metadata
                .iter()
                .any(|source| source.provider == provider)
        {
            return Err(Error::DependencyProvider(provider));
        }
        meter.charge_collection_slots(1, &path)?;
        meter.charge_owned_bytes(
            std::mem::size_of::<(ConeIdentity, &lir::CrossConeLirBridgeSectionV1)>() as u64,
            &path,
        )?;
        providers.insert(provider, *table);
    }
    for layouts in dependencies.layouts {
        meter.charge_work(u64::from(providers.len().max(1).ilog2()) + 1, &path)?;
        if !providers.contains_key(&layouts.provider()) {
            return Err(Error::DependencyProvider(layouts.provider()));
        }
    }
    let mut records = Vec::new();
    meter.check_table_entries(mir.selected().len() as u64, &path)?;
    meter.try_reserve_collection_slots(&mut records, mir.selected().len(), &path)?;
    for usage in mir.selected() {
        let provider = usage.provider();
        let declaration = usage.declaration();
        meter.charge_work(u64::from(providers.len().max(1).ilog2()) + 1, &path)?;
        let table = providers
            .get(&provider)
            .ok_or(Error::MissingSelectedProvider(provider))?;
        meter.charge_work(u64::from(table.exports().len().max(1).ilog2()) + 1, &path)?;
        let export = table
            .export(declaration)
            .ok_or(Error::MissingSelectedExport {
                provider,
                declaration,
            })?;
        let signature = export.abi_signature();
        let parameters = signature.signature().parameters().len() as u64;
        let arguments = signature.arguments().len() as u64;
        meter.charge_work(
            parameters
                .saturating_mul(2)
                .saturating_add(arguments)
                .saturating_add(1),
            &path,
        )?;
        if export.target() != usage.implementation() || signature.signature() != usage.signature() {
            return Err(Error::SelectedMirMismatch {
                provider,
                declaration,
            });
        }
        meter.charge_collection_slots(parameters.saturating_add(arguments), &path)?;
        meter.charge_owned_bytes(
            parameters
                .saturating_mul(std::mem::size_of::<PersistentExactTypeId>() as u64)
                .saturating_add(
                    arguments.saturating_mul(std::mem::size_of::<ScoopAbiArgument>() as u64),
                ),
            &path,
        )?;
        records.push(
            lir::SelectedDependencyLirCallableV1::new(
                provider,
                declaration,
                export.target(),
                signature.clone(),
                export.calling_convention(),
                export.root_plan(),
            )
            .map_err(|source| Error::Export {
                declaration,
                source: Box::new(source),
            })?,
        );
    }
    Ok(records)
}
