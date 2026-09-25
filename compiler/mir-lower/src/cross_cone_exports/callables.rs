use super::*;
use scoop_identity::DependencyCallableDeclarationId;

pub(super) fn combine<const N: usize>(
    tables: [mir::CanonicalMirCallableBindingsV1; N],
    ordinary: &mir::CrossConeMirBridgeSectionV1,
) -> Result<mir::CanonicalMirCallableBindingsV1, Error> {
    let count = tables.iter().try_fold(0usize, |count, table| {
        count
            .checked_add(table.entries().len())
            .ok_or(Error::CountOverflow)
    })?;
    let mut records = reserve(count)?;
    for table in tables {
        records.extend(table.into_entries());
    }

    // Detect duplicate implementations before removing the old partition.
    let combined =
        mir::CanonicalMirCallableBindingsV1::try_new(records).map_err(Error::Callables)?;
    let mut records = reserve(count)?;
    for record in combined.into_entries() {
        let declaration = match record.origin() {
            mir::MirCallableOriginV1::Function(id) => {
                Some(DependencyCallableDeclarationId::Function(*id))
            }
            mir::MirCallableOriginV1::Accessor(id) => {
                Some(DependencyCallableDeclarationId::PropertyAccessor(*id))
            }
            mir::MirCallableOriginV1::Constructor(_)
            | mir::MirCallableOriginV1::Generated { .. } => None,
        };
        if let Some(old) = declaration.and_then(|declaration| ordinary.export(declaration)) {
            if old.implementation() != record.implementation()
                || old.signature() != record.semantic_signature().exact()
                || old.signature() != record.lowered_signature().exact()
            {
                return Err(Error::OrdinaryCallableMismatch(record.implementation()));
            }
        } else {
            records.push(record);
        }
    }
    // Filtering preserves the canonical order, but the public constructor
    // deliberately checks it again at the final table boundary.

    mir::CanonicalMirCallableBindingsV1::try_new(records).map_err(Error::Callables)
}

pub(super) fn validate_ordinary(input: MirTypeBridgeExportInputV1<'_>) -> Result<(), Error> {
    let expected = hir::select_ordinary_source_callables(
        input.mir.module().cone,
        input.public,
        input.nominal_classifier,
        input.identities,
    )
    .map_err(|error| match error {
        hir::SharedTypeMetadataError::Resource(error) => Error::Resource(error),
        error => Error::OrdinarySource(Box::new(error)),
    })?;
    if expected.len() != input.ordinary.exports().len() {
        return Err(Error::IncompleteOrdinaryCallables {
            expected: expected.len(),
            actual: input.ordinary.exports().len(),
        });
    }
    for (declaration, source) in expected {
        let classified = input
            .nominal_classifier
            .classify_callable(source)
            .map_err(Error::OrdinaryClassification)?
            .ok_or(Error::OrdinaryCallableMismatch(
                declaration.implementation(),
            ))?;
        let actual = input
            .ordinary
            .export(declaration)
            .ok_or(Error::OrdinaryCallableMismatch(
                declaration.implementation(),
            ))?;
        if actual.implementation() != declaration.implementation()
            || actual.signature() != classified.signature()
        {
            return Err(Error::OrdinaryCallableMismatch(
                declaration.implementation(),
            ));
        }
    }
    Ok(())
}
