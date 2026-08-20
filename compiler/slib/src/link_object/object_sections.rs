//! Section roles and inventory shared by both builtin object formats.

use super::ValidatedObjectEnvelopeV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinLinkObjectSectionProfileV1 {
    ScoopLir,
    GeneratedCBridge,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuiltinObjectSectionRoleV1 {
    ObjectMetadata,
    Text,
    ReadOnlyData,
    CString,
    WritableData,
    ZeroFill,
    GccExceptionTable,
    LlvmStackmaps,
    CompactUnwind,
    EhFrame,
    ThreadLocalData,
    ThreadLocalZeroFill,
    ThreadLocalVariables,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedBuiltinObjectSectionInventoryV1 {
    pub(in crate::link_object) envelope: ValidatedObjectEnvelopeV1,
    pub(in crate::link_object) profile: BuiltinLinkObjectSectionProfileV1,
    pub(in crate::link_object) roles: Vec<BuiltinObjectSectionRoleV1>,
}

impl ValidatedBuiltinObjectSectionInventoryV1 {
    pub const fn envelope(&self) -> &ValidatedObjectEnvelopeV1 {
        &self.envelope
    }

    pub const fn profile(&self) -> BuiltinLinkObjectSectionProfileV1 {
        self.profile
    }

    pub fn roles(&self) -> &[BuiltinObjectSectionRoleV1] {
        &self.roles
    }
}
