use super::*;
use std::collections::HashMap;

pub(super) fn validate_exports<E>(
    consumer: ConeIdentity,
    target: crate::LirTargetProfile,
    dependencies: &[&LayoutAbiExportConstituentsV1],
) -> Result<(), LayoutAbiSectionError<E>> {
    let path = WirePath::root();
    let mut providers = std::collections::HashSet::new();

    for dependency in dependencies {
        let provider = dependency.provider();
        if provider == consumer || providers.contains(&provider) {
            return Err(LayoutAbiSectionError::DuplicateProvider(provider));
        }
        if dependency.target_profile() != target {
            return Err(LayoutAbiSectionError::DependencyTarget { provider });
        }
        scoop_wire::allocation::try_reserve_set(&mut providers, 1, &path)?;
        providers.insert(provider);
    }
    Ok(())
}

pub(crate) fn complete<'a, E>(
    consumer: ConeIdentity,
    target: crate::LirTargetProfile,
    direct: &[&'a CrossConeLayoutAbiSectionV1<'a>],
) -> Result<Vec<&'a CrossConeLayoutAbiSectionV1<'a>>, LayoutAbiSectionError<E>> {
    let path = WirePath::root();
    let mut pending = reserve(direct.len())?;
    pending.extend(direct.iter().map(|section| (*section, 1u64)));
    let mut sections: Vec<&'a CrossConeLayoutAbiSectionV1<'a>> = Vec::new();
    let mut by_provider = HashMap::new();
    while let Some((section, depth)) = pending.pop() {
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

        scoop_wire::allocation::try_reserve_map(&mut by_provider, 1, &path)?;
        scoop_wire::allocation::try_reserve(&mut sections, 1, &path)?;
        by_provider.insert(section.provider(), sections.len());
        sections.push(section);
        let next = depth
            .checked_add(1)
            .ok_or(LayoutAbiSemanticClosureError::ArithmeticOverflow)?;

        scoop_wire::allocation::try_reserve(&mut pending, section.dependencies.len(), &path)?;
        pending.extend(
            section
                .dependencies
                .iter()
                .map(|dependency| (*dependency, next)),
        );
    }

    sections.sort_unstable_by_key(|section| section.provider());
    Ok(sections)
}
