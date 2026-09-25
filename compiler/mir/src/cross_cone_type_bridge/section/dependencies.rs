use super::*;
use std::collections::HashMap;

pub(super) fn complete<'a, E>(
    consumer: ConeIdentity,
    direct: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
) -> Result<Vec<&'a CrossConeMirTypeBridgeSectionV1<'a>>, MirTypeBridgeSectionError<E>> {
    let path = WirePath::root();
    let mut first = reserve(direct.len())?;
    first.extend_from_slice(direct);

    first.sort_unstable_by_key(|section| section.provider());
    if let Some(pair) = first
        .windows(2)
        .find(|pair| pair[0].provider() == pair[1].provider())
    {
        return Err(MirTypeBridgeSectionError::DuplicateProvider(
            pair[0].provider(),
        ));
    }
    let mut pending = reserve(first.len())?;
    pending.extend(first);
    let mut sections: Vec<&'a CrossConeMirTypeBridgeSectionV1<'a>> = Vec::new();
    let mut by_provider: HashMap<ConeIdentity, usize> = HashMap::new();
    while let Some(section) = pending.pop() {
        if section.provider() == consumer {
            return Err(MirTypeBridgeSectionError::DuplicateProvider(consumer));
        }
        if let Some(index) = by_provider.get(&section.provider()).copied() {
            if !std::ptr::eq(sections[index], section) {
                return Err(MirTypeBridgeSectionError::DuplicateProvider(
                    section.provider(),
                ));
            }
            continue;
        }

        scoop_wire::allocation::try_reserve_map(&mut by_provider, 1, &path)?;
        scoop_wire::allocation::try_reserve(&mut sections, 1, &path)?;
        by_provider.insert(section.provider(), sections.len());
        sections.push(section);

        scoop_wire::allocation::try_reserve(&mut pending, section.dependencies.len(), &path)?;
        pending.extend(section.dependencies.iter().copied());
    }

    sections.sort_unstable_by_key(|section| section.provider());
    Ok(sections)
}

pub(super) fn types<'b, 'a: 'b, E>(
    local: &'b CanonicalParamFreeMirTypeExportsV1,
    dependencies: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
) -> Result<MirTypeBridgeTypeIndexV1<'b>, MirTypeBridgeSectionError<E>> {
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
    let mut tables = reserve(count)?;
    tables.push(local);
    tables.extend(dependencies.iter().map(|section| section.types()));
    Ok(MirTypeBridgeTypeIndexV1::try_new(&tables)?)
}
