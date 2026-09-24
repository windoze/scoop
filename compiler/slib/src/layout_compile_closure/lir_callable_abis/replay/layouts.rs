use std::collections::BTreeSet;

use scoop_identity::{ConeIdentity, PersistentExactTypeId, RepresentationRole};

use super::*;

pub(super) struct Layouts<'a> {
    local: &'a lir::CanonicalExactLayoutExportsV1,
    dependencies: &'a [&'a lir::CanonicalExactLayoutExportsV1],
}

impl<'a> Layouts<'a> {
    pub(super) fn new(
        local: &'a lir::CanonicalExactLayoutExportsV1,
        dependencies: &'a [&'a lir::CanonicalExactLayoutExportsV1],
        target: lir::LirTargetProfile,
        provider: ConeIdentity,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.charge_work(1, &path)?;
        if local.provider() != provider {
            return Err(Error::LocalProvider);
        }
        if local.target() != target {
            return Err(Error::LocalTarget);
        }
        let mut providers = BTreeSet::new();
        for table in dependencies {
            meter.charge_work(u64::from(providers.len().max(1).ilog2()) + 1, &path)?;
            let origin = table.provider();
            if origin == provider || providers.contains(&origin) {
                return Err(Error::DependencyProvider(origin));
            }
            if table.target() != target {
                return Err(Error::DependencyTarget(origin));
            }
            meter.charge_collection_slots(1, &path)?;
            meter.charge_owned_bytes(std::mem::size_of::<ConeIdentity>() as u64, &path)?;
            providers.insert(origin);
        }
        Ok(Self {
            local,
            dependencies,
        })
    }

    pub(super) fn value(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<&lir::ExactLayoutExportV1, lir::ExactCallableAbiError> {
        let mut found = None;
        for table in std::iter::once(self.local).chain(self.dependencies.iter().copied()) {
            meter.charge_work(table.records().len() as u64, &WirePath::root())?;
            if let Some(value) = table.find_exact_role(exact, RepresentationRole::ManagedValue) {
                if found.replace(value).is_some() {
                    return Err(lir::ExactCallableAbiError::DuplicateValueLayout { exact });
                }
            }
        }
        found.ok_or(lir::ExactCallableAbiError::MissingValueLayout { exact })
    }
}
