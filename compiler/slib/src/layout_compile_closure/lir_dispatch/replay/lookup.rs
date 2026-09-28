use super::*;
use scoop_identity::{
    CallableDefinitionOwner, ConeIdentity, PersistentExactTypeId, RepresentationRole,
};
use std::collections::BTreeSet;

pub(super) struct Abis<'a>(SharedLirDispatchAbiInputsV1<'a>, lir::LirTargetProfile);
impl<'a> Abis<'a> {
    pub(super) fn new(
        inputs: SharedLirDispatchAbiInputsV1<'a>,
        target: lir::LirTargetProfile,
        provider: ConeIdentity,
    ) -> Result<Self, Error> {
        if inputs.local_layouts.provider() != provider
            || inputs.local_callables.provider() != provider
            || inputs.local_direct_callables.artifact() != provider
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
        providers(
            inputs
                .dependency_direct_callables
                .iter()
                .map(|table| (table.artifact(), target)),
            provider,
            target,
        )?;
        Ok(Self(inputs, target))
    }
    pub(super) fn callable(
        &self,
        target: CallableDefinitionOwner,
    ) -> Result<lir::DispatchCallableAbiV1<'a>, Error> {
        let mut found = None;
        for table in std::iter::once(self.0.local_callables)
            .chain(self.0.dependency_callables.iter().copied())
        {
            if let Some(value) = table.get(target) {
                let receiver = match value
                    .canonical_signature()
                    .signature()
                    .receiver()
                    .into_option()
                {
                    Some(exact) => lir::CallableAbiReceiverInputV1::Receiver(self.value(exact)?),
                    None => lir::CallableAbiReceiverInputV1::NoReceiver,
                };
                if found
                    .replace(lir::DispatchCallableAbiV1::Exact {
                        record: value,
                        receiver,
                    })
                    .is_some()
                {
                    return Err(Error::DuplicateCallable(target));
                }
            }
        }
        for table in std::iter::once(self.0.local_direct_callables)
            .chain(self.0.dependency_direct_callables.iter().copied())
        {
            if let Some(record) = target
                .strong_owner()
                .and_then(|target| table.export_for_target(target))
            {
                if found.is_some() {
                    return Err(Error::DuplicateCallable(target));
                }
                let receiver = match record.abi_signature().signature().receiver().into_option() {
                    Some(exact) => lir::CallableAbiReceiverInputV1::Receiver(self.value(exact)?),
                    None => lir::CallableAbiReceiverInputV1::NoReceiver,
                };
                found = Some(lir::DispatchCallableAbiV1::Direct {
                    provider: table.artifact(),
                    target: self.1,
                    record,
                    receiver,
                });
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
                if found
                    .replace(value)
                    .is_some_and(|previous: &lir::ExactLayoutExportV1| {
                        !previous.has_same_odr_definition(value)
                    })
                {
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
