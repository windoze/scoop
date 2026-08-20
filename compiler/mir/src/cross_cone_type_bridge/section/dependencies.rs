use super::*;

pub(super) fn complete<'a>(
    consumer: ConeIdentity,
    dependencies: &[MirTypeBridgeDependencyViewV1<'a>],
) -> Result<Vec<MirTypeBridgeDependencyViewV1<'a>>, MirTypeBridgeSectionError> {
    let mut views = reserve(dependencies.len())?;
    views.extend_from_slice(dependencies);
    views.sort_unstable_by_key(|view| view.provider());
    if let Some(pair) = views
        .windows(2)
        .find(|pair| pair[0].provider() == pair[1].provider())
    {
        return Err(MirTypeBridgeSectionError::DuplicateProvider(
            pair[0].provider(),
        ));
    }
    if views.iter().any(|view| view.provider() == consumer) {
        return Err(MirTypeBridgeSectionError::DuplicateProvider(consumer));
    }
    Ok(views)
}

pub(super) fn types<'b, 'a: 'b>(
    local: &'b CanonicalParamFreeMirTypeExportsV1,
    dependencies: &[MirTypeBridgeDependencyViewV1<'a>],
) -> Result<MirTypeBridgeTypeIndexV1<'b>, MirTypeBridgeSectionError> {
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
    let mut tables = reserve(count)?;
    tables.push(local);
    tables.extend(dependencies.iter().map(|view| view.exports.types()));
    Ok(MirTypeBridgeTypeIndexV1::try_new(&tables)?)
}
