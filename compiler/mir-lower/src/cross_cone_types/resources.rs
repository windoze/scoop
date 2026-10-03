use super::*;

pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
) -> Result<(), SourceMirTypeProductionError> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(values, count, &path)
        .map_err(SourceMirTypeProductionError::Resource)
}
