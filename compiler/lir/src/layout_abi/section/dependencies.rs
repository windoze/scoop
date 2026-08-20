use super::*;

pub(super) fn validate_exports(
    consumer: ConeIdentity,
    target: crate::LirTargetProfile,
    dependencies: &[&LayoutAbiExportConstituentsV1],
) -> Result<(), LayoutAbiSectionError> {
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

pub(crate) fn complete<'a>(
    consumer: ConeIdentity,
    target: crate::LirTargetProfile,
    dependencies: &[&'a LayoutAbiExportConstituentsV1],
) -> Result<Vec<&'a LayoutAbiExportConstituentsV1>, LayoutAbiSectionError> {
    validate_exports(consumer, target, dependencies)?;
    let mut tables = reserve(dependencies.len())?;
    tables.extend_from_slice(dependencies);
    tables.sort_unstable_by_key(|table| table.provider());
    Ok(tables)
}
