use std::collections::BTreeMap;

use super::*;

pub(super) fn replay(
    mir: &mir::CrossConeMirBridgeSectionV1,
    dependencies: SharedOrdinaryLirBridgeDependenciesV1<'_>,
) -> Result<Vec<lir::SelectedDependencyLirCallableV1>, Error> {
    let path = WirePath::root();
    if dependencies.metadata.len() != dependencies.callables.len() {
        return Err(Error::DependencySources);
    }
    let mut providers = BTreeMap::new();
    for table in dependencies.callables {
        let provider = table.artifact();

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

        providers.insert(provider, *table);
    }
    for layouts in dependencies.layouts {
        if !providers.contains_key(&layouts.provider()) {
            return Err(Error::DependencyProvider(layouts.provider()));
        }
    }
    let mut records = Vec::new();

    scoop_wire::allocation::try_reserve(&mut records, mir.selected().len(), &path)?;
    for usage in mir.selected() {
        let provider = usage.provider();
        let declaration = usage.declaration();

        let table = providers
            .get(&provider)
            .ok_or(Error::MissingSelectedProvider(provider))?;

        let export = table
            .export(declaration)
            .ok_or(Error::MissingSelectedExport {
                provider,
                declaration,
            })?;
        let signature = export.abi_signature();

        if export.target() != usage.implementation() || signature.signature() != usage.signature() {
            return Err(Error::SelectedMirMismatch {
                provider,
                declaration,
            });
        }

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
