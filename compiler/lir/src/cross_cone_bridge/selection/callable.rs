//! Selected canonical callable data shared by all external LIR materialization.

use crate::{ParamFreeLirCallableExportV1, SelectedDependencyLirCallableV1};
pub use scoop_identity::CallableRole;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedExternalLirCallable {
    record: SelectedDependencyLirCallableV1,
    role: CallableRole,
}

impl SelectedExternalLirCallable {
    pub(super) fn dependency(record: SelectedDependencyLirCallableV1) -> Self {
        Self {
            record,
            role: CallableRole::Ordinary,
        }
    }
    pub(super) fn initialization_cycle(record: SelectedDependencyLirCallableV1) -> Self {
        Self {
            record,
            role: CallableRole::InitializationCycle,
        }
    }
    pub const fn record(&self) -> &SelectedDependencyLirCallableV1 {
        &self.record
    }
    pub const fn role(&self) -> CallableRole {
        self.role
    }
    pub const fn provider(&self) -> scoop_identity::ConeIdentity {
        self.record.provider()
    }
    pub const fn bridge(&self) -> &ParamFreeLirCallableExportV1 {
        self.record.bridge()
    }
    pub fn materialize(
        &self,
        signature: crate::ScoopAbiSignature,
    ) -> Result<crate::ExternalCallable, crate::ExternalCallableBuildError> {
        crate::ExternalCallable::new(self.record.clone(), signature)
    }
}
