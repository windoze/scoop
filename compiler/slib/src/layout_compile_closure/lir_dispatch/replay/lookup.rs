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
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        meter.charge_work(1, &WirePath::root())?;
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
            meter,
        )?;
        providers(
            inputs
                .dependency_callables
                .iter()
                .map(|table| (table.provider(), table.target())),
            provider,
            target,
            meter,
        )?;
        Ok(Self(inputs))
    }
    pub(super) fn callable(
        &self,
        target: StrongCallableDefinitionOwner,
        meter: &mut BudgetMeter,
    ) -> Result<&'a lir::ExactCallableAbiExportV1, Error> {
        let mut found = None;
        for table in std::iter::once(self.0.local_callables)
            .chain(self.0.dependency_callables.iter().copied())
        {
            meter.charge_work(
                u64::from(table.records().len().max(1).ilog2()) + 1,
                &WirePath::root(),
            )?;
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
        meter: &mut BudgetMeter,
    ) -> Result<&'a lir::ExactLayoutExportV1, Error> {
        let mut found = None;
        for table in
            std::iter::once(self.0.local_layouts).chain(self.0.dependency_layouts.iter().copied())
        {
            meter.charge_work(table.records().len() as u64, &WirePath::root())?;
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
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    let mut seen = BTreeSet::new();
    for (provider, profile) in tables {
        meter.charge_work(u64::from(seen.len().max(1).ilog2()) + 1, &path)?;
        if provider == current || seen.contains(&provider) {
            return Err(Error::DependencyProvider(provider));
        }
        if profile != target {
            return Err(Error::DependencyTarget(provider));
        }
        meter.charge_collection_slots(1, &path)?;
        meter.charge_owned_bytes(std::mem::size_of::<ConeIdentity>() as u64, &path)?;
        seen.insert(provider);
    }
    Ok(())
}
