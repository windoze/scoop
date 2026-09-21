//! Complete selected callable data shared by every external MIR use.

use crate::SelectedDependencyMirCallableV1;
use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, ExactCallableSignature,
    StrongCallableDefinitionOwner,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalCallableRole {
    Dependency,
    InitializationCycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedExternalMirCallable {
    record: SelectedDependencyMirCallableV1,
    role: ExternalCallableRole,
}

impl SelectedExternalMirCallable {
    pub(super) fn dependency(record: SelectedDependencyMirCallableV1) -> Self {
        Self {
            record,
            role: ExternalCallableRole::Dependency,
        }
    }
    pub(super) fn initialization_cycle(record: SelectedDependencyMirCallableV1) -> Self {
        Self {
            record,
            role: ExternalCallableRole::InitializationCycle,
        }
    }
    pub const fn record(&self) -> &SelectedDependencyMirCallableV1 {
        &self.record
    }
    pub const fn role(&self) -> ExternalCallableRole {
        self.role
    }
    pub const fn provider(&self) -> ConeIdentity {
        self.record.provider()
    }
    pub const fn declaration(&self) -> DependencyCallableDeclarationId {
        self.record.declaration()
    }
    pub const fn implementation(&self) -> StrongCallableDefinitionOwner {
        self.record.implementation()
    }
    pub const fn signature(&self) -> &ExactCallableSignature {
        self.record.signature()
    }
}
