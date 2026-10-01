use crate::*;
use scoop_identity::{CanonicalScoopStorage, RepresentationRole, ScoopAbiReturn};

pub(super) fn check(
    layouts: &CanonicalExactLayoutExportsV1,
    callables: &CanonicalExactCallableAbiExportsV1,
    ordinary: &CrossConeLirBridgeSectionV1,
    dependencies: &[&LayoutAbiExportConstituentsV1],
) -> Result<(), LinkDataError> {
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
            check_storage(argument.storage(), layouts, dependencies)?;
        }
        match signature.result() {
            ScoopAbiReturn::UnitVoid => continue,
            ScoopAbiReturn::ElidedZst(storage)
            | ScoopAbiReturn::Direct(storage)
            | ScoopAbiReturn::Indirect(storage) => {
                check_storage(storage, layouts, dependencies)?;
            }
        }
    }
    Ok(())
}

fn check_storage(
    storage: CanonicalScoopStorage,
    local: &CanonicalExactLayoutExportsV1,
    dependencies: &[&LayoutAbiExportConstituentsV1],
) -> Result<(), LinkDataError> {
    let exact = storage.exact_type();
    let layout = std::iter::once(local)
        .chain(dependencies.iter().map(|provider| provider.layouts()))
        .find_map(|layouts| layouts.find_exact_role(exact, RepresentationRole::ManagedValue))
        .ok_or_else(|| LinkDataError(format!("missing callable ABI value layout {exact}")))?;
    let value = layout
        .value_handle()
        .ok_or_else(|| LinkDataError(format!("callable ABI requires a value layout {exact}")))?;
    let actual = value.value().storage();
    let shape = match value.representation().kind() {
        ExactRepresentationKindV1::Scalar(_)
        | ExactRepresentationKindV1::QualifiedPointer(_)
        | ExactRepresentationKindV1::NicheEnum(_) => scoop_identity::ScoopAbiValueShape::Scalar,
        ExactRepresentationKindV1::IntrinsicValue(_)
        | ExactRepresentationKindV1::Tuple(_)
        | ExactRepresentationKindV1::Struct(_)
        | ExactRepresentationKindV1::TaggedEnum(_) => scoop_identity::ScoopAbiValueShape::Aggregate,
    };
    if storage.byte_size() != actual.byte_size()
        || storage.alignment().get() != actual.alignment().get()
        || storage.shape() != shape
    {
        return Err(LinkDataError(format!(
            "callable ABI storage differs from layout {exact}"
        )));
    }
    Ok(())
}
