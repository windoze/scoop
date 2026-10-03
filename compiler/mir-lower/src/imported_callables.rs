use super::*;

#[derive(Clone)]
pub(super) struct ImportedCallableTarget {
    pub(super) entry: ImportedCallableEntry,
    pub(super) lowering_role: mir::MirCallableLoweringRoleV1,
    pub(super) signature: scoop_identity::ExactCallableSignature,
    pub(super) semantic_signature: scoop_identity::ExactCallableSignature,
}

#[derive(Clone)]
pub(super) enum ImportedCallableEntry {
    Scoop {
        callable: mir::ExternalCallableUseId,
        native_contract: Option<scoop_identity::SourceNativeExternalContractRecord>,
    },
    ReleaseNative(scoop_identity::SourceNativeExternalContractRecord),
}

impl ImportedCallableTarget {
    pub(super) fn external(&self) -> Option<mir::ExternalCallableUseId> {
        match self.entry {
            ImportedCallableEntry::Scoop { callable, .. } => Some(callable),
            ImportedCallableEntry::ReleaseNative(_) => None,
        }
    }

    pub(super) fn scoop_entry(&self) -> mir::ExternalCallableUseId {
        self.external()
            .expect("an ordinary use retains its Scoop entry")
    }

    pub(super) fn native_contract(
        &self,
    ) -> Option<&scoop_identity::SourceNativeExternalContractRecord> {
        match &self.entry {
            ImportedCallableEntry::Scoop {
                native_contract, ..
            } => native_contract.as_ref(),
            ImportedCallableEntry::ReleaseNative(contract) => Some(contract),
        }
    }
}
