use super::*;
use scoop_identity::{
    ConeIdentity, PersistentExactTypeId, RepresentationRole, StrongCallableDefinitionOwner,
};
use std::collections::BTreeSet;

pub(super) struct Abis<'a>(SharedLirDispatchAbiInputsV1<'a>);
impl<'a> Abis<'a> {
    pub(super) fn new(
        inputs: SharedLirDispatchAbiInputsV1<'a>,
        target: lir::LirTargetProfile,
        provider: ConeIdentity,
    ) -> Result<Self, Error> {
        if inputs.local_layouts.provider() != provider
            || inputs.local_callables.provider() != provider
        {
            return Err(Error::LocalProvider);
        }
        if inputs.local_layouts.target() != target || inputs.local_callables.target() != target {
            return Err(Error::LocalTarget);
        }
        providers(
            inputs
                .dependency_layouts
                .iter()
                .map(|table| (table.provider(), table.target())),
            provider,
            target,
        )?;
        providers(
            inputs
                .dependency_callables
                .iter()
                .map(|table| (table.provider(), table.target())),
            provider,
            target,
        )?;
        Ok(Self(inputs))
    }
    pub(super) fn callable(
        &self,
        target: StrongCallableDefinitionOwner,
    ) -> Result<&'a lir::ExactCallableAbiExportV1, Error> {
        let mut found = None;
        for table in std::iter::once(self.0.local_callables)
            .chain(self.0.dependency_callables.iter().copied())
        {
            if let Some(value) = table.get(target) {
                if found.replace(value).is_some() {
                    return Err(Error::DuplicateCallable(target));
                }
            }
        }
        found.ok_or(Error::MissingCallable(target))
    }
    pub(super) fn value(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&'a lir::ExactLayoutExportV1, Error> {
        let mut found = None;
        for table in
            std::iter::once(self.0.local_layouts).chain(self.0.dependency_layouts.iter().copied())
        {
            if let Some(value) = table.find_exact_role(exact, RepresentationRole::ManagedValue) {
                if found.replace(value).is_some() {
                    return Err(Error::DuplicateValueLayout(exact));
                }
            }
        }
        found.ok_or(Error::MissingValueLayout(exact))
    }
}

fn providers(
    tables: impl Iterator<Item = (ConeIdentity, lir::LirTargetProfile)>,
    current: ConeIdentity,
    target: lir::LirTargetProfile,
) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for (provider, profile) in tables {
        if provider == current || seen.contains(&provider) {
            return Err(Error::DependencyProvider(provider));
        }
        if profile != target {
            return Err(Error::DependencyTarget(provider));
        }

        seen.insert(provider);
    }
    Ok(())
}
