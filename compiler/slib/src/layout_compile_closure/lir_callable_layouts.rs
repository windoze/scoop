use std::collections::BTreeSet;

use scoop_identity::{
    ConeIdentity, ExactCallableSignature, PersistentExactTypeId, RepresentationRole,
};
use scoop_lir as lir;
use scoop_wire::{BudgetMeter, WirePath};

use super::SharedLirCallableAbiValidationError as Error;

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
        self.value_if_present(exact, meter)?
            .ok_or(lir::ExactCallableAbiError::MissingValueLayout { exact })
    }

    pub(super) fn value_if_present(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<Option<&lir::ExactLayoutExportV1>, lir::ExactCallableAbiError> {
        let mut found = None;
        for table in std::iter::once(self.local).chain(self.dependencies.iter().copied()) {
            meter.charge_work(table.records().len() as u64, &WirePath::root())?;
            if let Some(value) = table.find_exact_role(exact, RepresentationRole::ManagedValue) {
                if found.replace(value).is_some() {
                    return Err(lir::ExactCallableAbiError::DuplicateValueLayout { exact });
                }
            }
        }
        Ok(found)
    }

    pub(super) fn signature(
        &self,
        signature: &ExactCallableSignature,
        meter: &mut BudgetMeter,
    ) -> Result<SignatureLayouts<'_>, lir::ExactCallableAbiError> {
        let path = WirePath::root();
        meter.charge_work(signature.parameters().len() as u64 + 2, &path)?;
        meter.charge_edges(signature.parameters().len() as u64 + 2, &path)?;
        let receiver = match signature.receiver().into_option() {
            None => lir::CallableAbiReceiverInputV1::NoReceiver,
            Some(exact) => lir::CallableAbiReceiverInputV1::Receiver(self.value(exact, meter)?),
        };
        let mut parameters = Vec::new();
        meter.try_reserve_collection_slots(&mut parameters, signature.parameters().len(), &path)?;
        for exact in signature.parameters() {
            parameters.push(self.value(*exact, meter)?);
        }
        let result = self.value(signature.result(), meter)?;
        Ok(SignatureLayouts {
            receiver,
            parameters,
            result,
        })
    }
}

pub(super) struct SignatureLayouts<'a> {
    receiver: lir::CallableAbiReceiverInputV1<'a>,
    parameters: Vec<&'a lir::ExactLayoutExportV1>,
    result: &'a lir::ExactLayoutExportV1,
}

impl SignatureLayouts<'_> {
    pub(super) fn inputs(&self) -> lir::CallableAbiLayoutInputsV1<'_> {
        lir::CallableAbiLayoutInputsV1 {
            receiver: self.receiver,
            parameters: &self.parameters,
            result: self.result,
        }
    }
}

pub(super) fn clone_signature(
    signature: &ExactCallableSignature,
    meter: &mut BudgetMeter,
) -> Result<ExactCallableSignature, scoop_wire::WireError> {
    let count = signature.parameters().len() as u64;
    let path = WirePath::root();
    meter.charge_work(count, &path)?;
    meter.charge_collection_slots(count, &path)?;
    meter.charge_owned_bytes(
        count.saturating_mul(std::mem::size_of::<PersistentExactTypeId>() as u64),
        &path,
    )?;
    Ok(signature.clone())
}
