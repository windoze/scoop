use super::*;
use scoop_identity::RepresentationRole;
use std::collections::BTreeSet;

pub(super) struct Layouts<'a> {
    pub local: &'a lir::CanonicalExactLayoutExportsV1,
    pub dependencies: &'a [&'a lir::CanonicalExactLayoutExportsV1],
}

impl Layouts<'_> {
    pub fn value(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<&lir::ExactLayoutExportV1, Error> {
        let mut found = None;
        for table in std::iter::once(self.local).chain(self.dependencies.iter().copied()) {
            meter.charge_work(table.records().len() as u64, &WirePath::root())?;
            if let Some(record) = table.find_exact_role(exact, RepresentationRole::ManagedValue) {
                if found.is_some() {
                    return Err(Error::AmbiguousLayout(exact));
                }
                found = Some(record);
            }
        }
        found.ok_or(Error::MissingLayout(exact))
    }
}

pub(super) fn validate_dependencies(
    provider: ConeIdentity,
    target: lir::LirTargetProfile,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut providers = BTreeSet::new();
    for (role, origin, actual) in dependencies
        .layouts
        .iter()
        .map(|table| (0, table.provider(), table.target()))
        .chain(
            dependencies
                .callables
                .iter()
                .map(|table| (1, table.provider(), table.target())),
        )
    {
        meter.charge_collection_slots(1, &WirePath::root())?;
        search(providers.len(), meter)?;
        if origin == provider {
            return Err(Error::Provider);
        }
        if actual != target {
            return Err(Error::Target);
        }
        if !providers.insert((role, origin)) {
            return Err(Error::DuplicateProvider(origin));
        }
    }
    Ok(())
}

pub(super) fn callable<'a>(
    target: StrongCallableDefinitionOwner,
    local: &'a lir::CanonicalExactCallableAbiExportsV1,
    dependencies: &'a [&'a lir::CanonicalExactCallableAbiExportsV1],
    meter: &mut BudgetMeter,
) -> Result<&'a lir::ExactCallableAbiExportV1, Error> {
    let mut found = None;
    for table in std::iter::once(local).chain(dependencies.iter().copied()) {
        search(table.records().len(), meter)?;
        if let Some(record) = table.get(target) {
            if found.is_some() {
                return Err(Error::AmbiguousCallable(target));
            }
            found = Some(record);
        }
    }
    found.ok_or(Error::MissingCallable(target))
}
