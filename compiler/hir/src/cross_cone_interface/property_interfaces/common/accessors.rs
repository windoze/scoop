use scoop_identity::PersistentPropertyAccessorId;

use super::PropertyCapabilityBuildError;

mod source;
mod wire;
pub use source::{PropertyAccessorImplementationV1, PropertyAccessorSourceV1};
pub use wire::DecodedPropertyAccessorsV1;

/// Complete accessor identities and source forms, independent of public lookup.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PropertyAccessorsV1 {
    getter: PropertyAccessorSourceV1,
    setter: Option<PropertyAccessorSourceV1>,
}

impl PropertyAccessorsV1 {
    pub const fn read_only(getter: PropertyAccessorSourceV1) -> Self {
        Self {
            getter,
            setter: None,
        }
    }

    pub fn try_read_write(
        getter: PropertyAccessorSourceV1,
        setter: PropertyAccessorSourceV1,
    ) -> Result<Self, PropertyCapabilityBuildError> {
        if getter.accessor() == setter.accessor() {
            return Err(PropertyCapabilityBuildError::DuplicateAccessor(
                getter.accessor(),
            ));
        }
        Ok(Self {
            getter,
            setter: Some(setter),
        })
    }

    pub const fn getter(self) -> PersistentPropertyAccessorId {
        self.getter.accessor()
    }

    pub const fn setter(self) -> Option<PersistentPropertyAccessorId> {
        match self.setter {
            Some(setter) => Some(setter.accessor()),
            None => None,
        }
    }

    pub const fn getter_source(self) -> PropertyAccessorSourceV1 {
        self.getter
    }

    pub const fn setter_source(self) -> Option<PropertyAccessorSourceV1> {
        self.setter
    }

    pub const fn is_read_only(self) -> bool {
        self.setter.is_none()
    }
}
