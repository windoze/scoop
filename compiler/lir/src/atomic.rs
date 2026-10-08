use crate::{AtomicValueKind, LirType, MANAGED_PTR, Value};

/// The hidden value field of a source atomic object. The address is formed
/// only while emitting the operation and never crosses a safepoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicLocation {
    pub object: Value,
    pub offset: u64,
    pub kind: AtomicValueKind,
}

impl AtomicLocation {
    pub const fn value_type(self) -> LirType {
        match self.kind {
            AtomicValueKind::Int => LirType::I32,
            AtomicValueKind::Long => LirType::I64,
            AtomicValueKind::Boolean => LirType::I1,
            AtomicValueKind::Reference => MANAGED_PTR,
        }
    }
}
