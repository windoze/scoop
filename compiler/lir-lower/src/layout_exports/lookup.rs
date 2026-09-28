use super::*;
use scoop_identity::RepresentationRole;
use std::collections::BTreeSet;

pub(super) struct Layouts<'a> {
    pub local: &'a lir::CanonicalExactLayoutExportsV1,
    pub dependencies: &'a [&'a lir::CanonicalExactLayoutExportsV1],
}

impl Layouts<'_> {
    pub fn value(&self, exact: PersistentExactTypeId) -> Result<&lir::ExactLayoutExportV1, Error> {
        let mut found = None;
        for table in std::iter::once(self.local).chain(self.dependencies.iter().copied()) {
            if let Some(record) = table.find_exact_role(exact, RepresentationRole::ManagedValue) {
                if found.is_some_and(|previous: &lir::ExactLayoutExportV1| {
                    !previous.has_same_odr_definition(record)
                }) {
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
    for table in dependencies.direct_callables {
        if table.artifact() == provider {
            return Err(Error::Provider);
        }
        if !providers.insert((2, table.artifact())) {
            return Err(Error::DuplicateProvider(table.artifact()));
        }
    }
    Ok(())
}

pub(super) fn callable<'a>(
    target: CallableDefinitionOwner,
    local: &'a lir::CanonicalExactCallableAbiExportsV1,
    dependencies: &'a [&'a lir::CanonicalExactCallableAbiExportsV1],
    direct: &'a lir::CrossConeLirBridgeSectionV1,
    direct_dependencies: &'a [&'a lir::CrossConeLirBridgeSectionV1],
    layouts: &'a Layouts<'_>,
) -> Result<lir::DispatchCallableAbiV1<'a>, Error> {
    let mut found = None;
    for table in std::iter::once(local).chain(dependencies.iter().copied()) {
        if let Some(record) = table.get(target) {
            if found.is_some() {
                return Err(Error::AmbiguousCallable(target));
            }
            let receiver = match record
                .canonical_signature()
                .signature()
                .receiver()
                .into_option()
            {
                Some(exact) => lir::CallableAbiReceiverInputV1::Receiver(layouts.value(exact)?),
                None => lir::CallableAbiReceiverInputV1::NoReceiver,
            };
            found = Some(lir::DispatchCallableAbiV1::Exact { record, receiver });
        }
    }
    for table in std::iter::once(direct).chain(direct_dependencies.iter().copied()) {
        if let Some(record) = target
            .strong_owner()
            .and_then(|target| table.export_for_target(target))
        {
            if found.is_some() {
                return Err(Error::AmbiguousCallable(target));
            }
            let receiver = match record.abi_signature().signature().receiver().into_option() {
                Some(exact) => lir::CallableAbiReceiverInputV1::Receiver(layouts.value(exact)?),
                None => lir::CallableAbiReceiverInputV1::NoReceiver,
            };
            found = Some(lir::DispatchCallableAbiV1::Direct {
                provider: table.artifact(),
                target: local.target(),
                record,
                receiver,
            });
        }
    }
    found.ok_or(Error::MissingCallable(target))
}
