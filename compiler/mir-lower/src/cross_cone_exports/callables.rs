use super::*;

pub(super) fn combine<const N: usize>(
    tables: [mir::CanonicalMirCallableBindingsV1; N],
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
    mir::CanonicalMirCallableBindingsV1::try_new(records).map_err(Error::Callables)
}
