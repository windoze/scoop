use super::*;
use std::collections::HashMap;

pub(crate) fn complete<'a, E>(
    consumer: ConeIdentity,
    target: crate::LirTargetProfile,
    direct: &[&'a CrossConeLayoutAbiSectionV1<'a>],
    meter: &mut BudgetMeter,
) -> Result<Vec<&'a CrossConeLayoutAbiSectionV1<'a>>, LayoutAbiSectionError<E>> {
    let path = WirePath::root();
    let mut pending = reserve(direct.len(), meter)?;
    pending.extend(direct.iter().map(|section| (*section, 1u64)));
    let mut sections: Vec<&'a CrossConeLayoutAbiSectionV1<'a>> = Vec::new();
    let mut by_provider = HashMap::new();
    while let Some((section, depth)) = pending.pop() {
        meter.charge_work(1, &path)?;
        if section.provider() == consumer {
            return Err(LayoutAbiSectionError::DuplicateProvider(consumer));
        }
        if section.target_profile() != target {
            return Err(LayoutAbiSectionError::DependencyTarget {
                provider: section.provider(),
            });
        }
        if let Some(index) = by_provider.get(&section.provider()).copied() {
            let existing: &'a CrossConeLayoutAbiSectionV1<'a> = sections[index];
            if !std::ptr::eq(existing, section) {
                return Err(LayoutAbiSectionError::DuplicateProvider(section.provider()));
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
        let next = depth
            .checked_add(1)
            .ok_or(LayoutAbiSemanticClosureError::ArithmeticOverflow)?;
        meter.charge_edges(section.dependencies.len() as u64, &path)?;
        meter.try_reserve_collection_slots(&mut pending, section.dependencies.len(), &path)?;
        pending.extend(
            section
                .dependencies
                .iter()
                .map(|dependency| (*dependency, next)),
        );
    }
    sort_work(sections.len(), meter)?;
    sections.sort_unstable_by_key(|section| section.provider());
    Ok(sections)
}
