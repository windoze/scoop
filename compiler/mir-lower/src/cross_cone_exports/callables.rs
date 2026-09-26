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
        for record in table.into_entries() {
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
            if let Some(existing) = declaration.and_then(|declaration| ordinary.export(declaration))
            {
                if existing.implementation() != record.implementation()
                    || existing.bridge_signature() != record.semantic_signature()
                    || existing.bridge_signature() != record.lowered_signature()
                {
                    return Err(Error::OrdinaryCallableMismatch(record.implementation()));
                }
            } else {
                records.push(record);
            }
        }
    }
    mir::CanonicalMirCallableBindingsV1::try_new(records).map_err(Error::Callables)
}
