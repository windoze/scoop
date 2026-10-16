//! Immutable codegen inputs and canonical object member planning.

use super::*;

/// Immutable bytes for one codegen member after its stable `.slib` identity
/// has been derived from the exact producer unit set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedScoopLirObjectInputV1 {
    pub(in crate::object_production) plan: PlannedScoopLirObjectMemberV1,
    pub(in crate::object_production) bytes: Vec<u8>,
}

/// Immutable generated-C bytes after the exact singleton bridge unit has
/// been bound to its stable `.slib` member identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedGeneratedCBridgeObjectInputV1 {
    pub(in crate::object_production) plan: PlannedGeneratedBridgeObjectMemberV1,
    pub(in crate::object_production) bytes: Vec<u8>,
}
