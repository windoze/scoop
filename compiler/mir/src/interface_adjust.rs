//! Typed interface receiver adapters and their physical table locations.

use crate::{ClassId, FunctionId, InterfaceId};
mod identity;
pub use identity::{InterfaceAdjustIdentity, InterfaceAdjustIdentityError};

/// Exact physical itable location materializing a boxing adjust identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InterfaceAdjustLocation {
    class: ClassId,
    interface: InterfaceId,
    slot: u32,
    function: FunctionId,
}

impl InterfaceAdjustLocation {
    pub const fn new(
        class: ClassId,
        interface: InterfaceId,
        slot: u32,
        function: FunctionId,
    ) -> Self {
        Self {
            class,
            interface,
            slot,
            function,
        }
    }

    pub const fn class(self) -> ClassId {
        self.class
    }

    pub const fn interface(self) -> InterfaceId {
        self.interface
    }

    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn function(self) -> FunctionId {
        self.function
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterfaceAdjustTarget {
    Local(FunctionId),
    External(crate::ExternalCallableUseId),
}

/// One checked physical boxing-adjust materialization.
#[derive(Clone, Debug)]
pub struct InterfaceAdjust {
    location: InterfaceAdjustLocation,
    target: InterfaceAdjustTarget,
    identity: InterfaceAdjustIdentity,
}

impl InterfaceAdjust {
    pub const fn new(
        location: InterfaceAdjustLocation,
        target: InterfaceAdjustTarget,
        identity: InterfaceAdjustIdentity,
    ) -> Self {
        Self {
            location,
            target,
            identity,
        }
    }

    pub const fn location(&self) -> InterfaceAdjustLocation {
        self.location
    }

    pub const fn class(&self) -> ClassId {
        self.location.class()
    }

    pub const fn interface(&self) -> InterfaceId {
        self.location.interface()
    }

    pub const fn slot(&self) -> u32 {
        self.location.slot()
    }

    pub const fn function(&self) -> FunctionId {
        self.location.function()
    }

    /// The concrete conformance target used when lowering the thunk body.
    pub const fn target(&self) -> InterfaceAdjustTarget {
        self.target
    }

    pub const fn identity(&self) -> &InterfaceAdjustIdentity {
        &self.identity
    }
}

#[cfg(test)]
mod tests;
