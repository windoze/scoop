use std::collections::BTreeSet;

use scoop_identity::{
    ConeIdentity, ExactCallableSignature, PersistentExactTypeId, RepresentationRole,
};
use scoop_lir as lir;
use scoop_wire::WirePath;

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

    pub(super) fn signature(
        &self,
        signature: &ExactCallableSignature,
    ) -> Result<SignatureLayouts<'_>, lir::ExactCallableAbiError> {
        let path = WirePath::root();

        let receiver = match signature.receiver().into_option() {
            None => lir::CallableAbiReceiverInputV1::NoReceiver,
            Some(exact) => lir::CallableAbiReceiverInputV1::Receiver(self.value(exact)?),
        };
        let mut parameters = Vec::new();
        scoop_wire::allocation::try_reserve(&mut parameters, signature.parameters().len(), &path)?;
        for exact in signature.parameters() {
            parameters.push(self.value(*exact)?);
        }
        let result = self.value(signature.result())?;
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
