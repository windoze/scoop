use crate::link_data::value_storage::{LinkLayouts, ValueStorageReader};
use crate::*;
use scoop_identity::{CanonicalScoopStorage, ScoopAbiReturn, ValidatedIdentityGraph};

pub(super) fn check(
    layouts: &CanonicalExactLayoutExportsV1,
    callables: &CanonicalExactCallableAbiExportsV1,
    ordinary: &CrossConeLirBridgeSectionV1,
    dependencies: &[&LayoutAbiExportConstituentsV1],
    identities: &ValidatedIdentityGraph,
) -> Result<(), LinkDataError> {
    let available = std::iter::once(layouts)
        .chain(dependencies.iter().map(|provider| provider.layouts()))
        .flat_map(|layouts| layouts.records())
        .map(|layout| (layout.identity().layout(), layout.clone()))
        .collect::<LinkLayouts>();
    let mut values = ValueStorageReader::default();
    for signature in callables
        .records()
        .iter()
        .map(ExactCallableAbiExportV1::canonical_signature)
        .chain(
            ordinary
                .exports()
                .iter()
                .map(ParamFreeLirCallableExportV1::abi_signature),
        )
        .chain(
            ordinary
                .selected()
                .iter()
                .map(|record| record.bridge().abi_signature()),
        )
    {
        for argument in signature.arguments() {
            check_storage(
                argument.storage(),
                layouts.target(),
                identities,
                &available,
                &mut values,
            )?;
        }
        match signature.result() {
            ScoopAbiReturn::UnitVoid => continue,
            ScoopAbiReturn::ElidedZst(storage)
            | ScoopAbiReturn::Direct(storage)
            | ScoopAbiReturn::DirectParts(storage)
            | ScoopAbiReturn::Indirect(storage) => {
                check_storage(
                    storage,
                    layouts.target(),
                    identities,
                    &available,
                    &mut values,
                )?;
            }
        }
    }
    Ok(())
}

fn check_storage(
    storage: CanonicalScoopStorage,
    target: LirTargetProfile,
    identities: &ValidatedIdentityGraph,
    layouts: &LinkLayouts,
    values: &mut ValueStorageReader,
) -> Result<(), LinkDataError> {
    let exact = storage.exact_type();
    let value = values.read(exact, target, identities, layouts)?;
    let actual = value.value.storage();
    if storage.byte_size() != actual.byte_size()
        || storage.alignment().get() != actual.alignment().get()
        || storage.shape() != value.shape
    {
        return Err(LinkDataError(format!(
            "callable ABI storage differs from layout {exact}"
        )));
    }
    Ok(())
}
