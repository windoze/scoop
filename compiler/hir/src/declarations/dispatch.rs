use super::*;

/// Checked dispatch for a callable definition. Interface members retain the
/// original slot identity independently of declaration storage.
#[derive(Debug, Clone, Copy)]
pub enum DeclaredMethodDispatch {
    Direct,
    Virtual(VirtualMethodId),
    FinalOverride(VirtualMethodId),
    Interface(scoop_identity::PersistentDispatchSlotId),
}
