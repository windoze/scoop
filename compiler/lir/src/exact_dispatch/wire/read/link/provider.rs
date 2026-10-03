use super::*;

pub(super) struct AbiReader<'a> {
    pub target: LirTargetProfile,
    pub layouts: &'a CanonicalExactLayoutExportsV1,
    pub callables: &'a CanonicalExactCallableAbiExportsV1,
    pub ordinary: &'a CrossConeLirBridgeSectionV1,
    pub dependencies: &'a [&'a LayoutAbiExportConstituentsV1],
}

impl<'a> AbiReader<'a> {
    pub fn layout(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&'a ExactLayoutExportV1, LinkDataError> {
        std::iter::once(self.layouts)
            .chain(self.dependencies.iter().map(|provider| provider.layouts()))
            .find_map(|layouts| layouts.find_exact_role(exact, RepresentationRole::ManagedValue))
            .ok_or_else(|| LinkDataError(format!("missing dispatch receiver layout {exact}")))
    }

    pub fn callable(
        &self,
        provider: ConeIdentity,
        target: CallableDefinitionOwner,
    ) -> Result<DispatchCallableAbiV1<'a>, LinkDataError> {
        let (callables, ordinary) = if provider == self.callables.provider() {
            (self.callables, self.ordinary)
        } else {
            let dependency = self
                .dependencies
                .iter()
                .find(|dependency| dependency.provider() == provider)
                .ok_or_else(|| LinkDataError(format!("missing dispatch provider {provider}")))?;
            (dependency.callables(), dependency.direct_callables())
        };
        if let Some(record) = callables.get(target) {
            return Ok(DispatchCallableAbiV1::Exact {
                record,
                receiver: self.receiver(record.canonical_signature())?,
            });
        }
        let record = target
            .strong_owner()
            .and_then(|target| ordinary.export_for_target(target))
            .ok_or_else(|| LinkDataError(format!("missing dispatch callable ABI {target:?}")))?;
        Ok(DispatchCallableAbiV1::Direct {
            provider,
            target: self.target,
            record,
            receiver: self.receiver(record.abi_signature())?,
        })
    }

    fn receiver(
        &self,
        signature: &scoop_identity::CanonicalScoopAbiFunctionSignature,
    ) -> Result<CallableAbiReceiverInputV1<'a>, LinkDataError> {
        match signature.signature().receiver().into_option() {
            None => Ok(CallableAbiReceiverInputV1::NoReceiver),
            Some(exact) => self.layout(exact).map(CallableAbiReceiverInputV1::Receiver),
        }
    }
}
