use super::*;
use std::collections::HashMap;

pub(super) fn complete<'a, E>(
    consumer: ConeIdentity,
    direct: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
    meter: &mut BudgetMeter,
) -> Result<Vec<&'a CrossConeMirTypeBridgeSectionV1<'a>>, MirTypeBridgeSectionError<E>> {
    let path = WirePath::root();
    let mut first = reserve(direct.len(), meter)?;
    first.extend_from_slice(direct);
    sort_work(first.len(), meter)?;
    first.sort_unstable_by_key(|section| section.provider());
    if let Some(pair) = first
        .windows(2)
        .find(|pair| pair[0].provider() == pair[1].provider())
    {
        return Err(MirTypeBridgeSectionError::DuplicateProvider(
            pair[0].provider(),
        ));
    }
    let mut pending = reserve(first.len(), meter)?;
    pending.extend(first.into_iter().map(|section| (section, 1u64)));
    let mut sections: Vec<&'a CrossConeMirTypeBridgeSectionV1<'a>> = Vec::new();
    let mut by_provider: HashMap<ConeIdentity, usize> = HashMap::new();
    while let Some((section, depth)) = pending.pop() {
        meter.charge_work(1, &path)?;
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
        meter.check_semantic_depth(depth, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.check_table_entries(sections.len() as u64 + 1, &path)?;
        meter.try_reserve_map_slots(&mut by_provider, 1, &path)?;
        meter.try_reserve_collection_slots(&mut sections, 1, &path)?;
        by_provider.insert(section.provider(), sections.len());
        sections.push(section);
        let next_depth = depth
            .checked_add(1)
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        meter.charge_edges(section.dependencies.len() as u64, &path)?;
        meter.try_reserve_collection_slots(&mut pending, section.dependencies.len(), &path)?;
        pending.extend(
            section
                .dependencies
                .iter()
                .map(|dependency| (*dependency, next_depth)),
        );
    }
    sort_work(sections.len(), meter)?;
    sections.sort_unstable_by_key(|section| section.provider());
    Ok(sections)
}

pub(super) fn types<'b, 'a: 'b, E>(
    local: &'b CanonicalParamFreeMirTypeExportsV1,
    dependencies: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
    meter: &mut BudgetMeter,
) -> Result<MirTypeBridgeTypeIndexV1<'b>, MirTypeBridgeSectionError<E>> {
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
    let mut tables = reserve(count, meter)?;
    tables.push(local);
    tables.extend(dependencies.iter().map(|section| section.types()));
    Ok(MirTypeBridgeTypeIndexV1::try_new(&tables, meter)?)
}
