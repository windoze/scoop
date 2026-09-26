use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, PersistentExactTypeId, RepresentationRole};
use scoop_lir as lir;

use super::SharedLirCallableAbiValidationError as Error;

mod signatures;

pub(super) struct Layouts<'a> {
    local: &'a lir::CanonicalExactLayoutExportsV1,
    dependencies: &'a [&'a lir::CanonicalExactLayoutExportsV1],
    values: BTreeMap<
        PersistentExactTypeId,
        (
            lir::ValueLayoutConstituentV1,
            scoop_identity::ScoopAbiValueShape,
        ),
    >,
}

impl<'a> Layouts<'a> {
    pub(super) fn new(
        local: &'a lir::CanonicalExactLayoutExportsV1,
        dependencies: &'a [&'a lir::CanonicalExactLayoutExportsV1],
        target: lir::LirTargetProfile,
        provider: ConeIdentity,
    ) -> Result<Self, Error> {
        if local.provider() != provider {
            return Err(Error::LocalProvider);
        }
        if local.target() != target {
            return Err(Error::LocalTarget);
        }
        let mut providers = BTreeSet::new();
        for table in dependencies {
            let origin = table.provider();
            if origin == provider || providers.contains(&origin) {
                return Err(Error::DependencyProvider(origin));
            }
            if table.target() != target {
                return Err(Error::DependencyTarget(origin));
            }

            providers.insert(origin);
        }
        Ok(Self {
            local,
            dependencies,
            values: BTreeMap::new(),
        })
    }

    pub(super) fn value(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&lir::ExactLayoutExportV1, lir::ExactCallableAbiError> {
        self.value_if_present(exact)?
            .ok_or(lir::ExactCallableAbiError::MissingValueLayout { exact })
    }

    pub(super) fn value_if_present(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<Option<&lir::ExactLayoutExportV1>, lir::ExactCallableAbiError> {
        let mut found = None;
        for table in std::iter::once(self.local).chain(self.dependencies.iter().copied()) {
            if let Some(value) = table.find_exact_role(exact, RepresentationRole::ManagedValue) {
                if found.replace(value).is_some() {
                    return Err(lir::ExactCallableAbiError::DuplicateValueLayout { exact });
                }
            }
        }
        Ok(found)
    }
}
