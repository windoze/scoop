use super::*;

pub(super) fn validate(
    records: &[ExternalHirReferenceV1],

    path: &WirePath,
) -> Result<(), ExternalHirReferenceSetBuildError> {
    use ExternalHirReferenceSetBuildError as Error;
    let mut positions = Vec::new();

    for record in records {
        for site in record.call_sites().records() {
            scoop_wire::allocation::try_reserve(&mut positions, 1, path)
                .map_err(Error::Resource)?;
            positions.push(site.position());
        }
    }

    positions.sort_unstable();
    if let Some(pair) = positions.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(Error::DuplicateCallPosition(pair[1]));
    }
    Ok(())
}
