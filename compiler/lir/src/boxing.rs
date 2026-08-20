//! Descriptor-refined boxing and unboxing operations.

use super::*;

mod reference;
mod refinement;
pub(crate) use reference::BoxedDescriptorReference;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxedZstDescriptor {
    descriptor: BoxedDescriptorReference,
    descriptor_exact: scoop_identity::PersistentExactTypeId,
    value: LogicalZstValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxedNonZeroDescriptor {
    descriptor: BoxedDescriptorReference,
    descriptor_exact: scoop_identity::PersistentExactTypeId,
    payload_exact: scoop_identity::PersistentExactTypeId,
    value: AbiValue,
}

/// Only a validated BoxedValue descriptor can enter a boxing operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoxedValueDescriptor {
    ZeroSized(BoxedZstDescriptor),
    NonZero(BoxedNonZeroDescriptor),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoxDescriptorError {
    InvalidDescriptor,
    NotBoxedValue,
    PayloadIdentityMismatch,
    InvalidValueLayout,
    StorageMismatch,
}

impl BoxedZstDescriptor {
    pub const fn descriptor(&self) -> TypeDescriptorRef {
        self.descriptor.reference()
    }

    pub const fn descriptor_exact(&self) -> scoop_identity::PersistentExactTypeId {
        self.descriptor_exact
    }

    pub const fn value(&self) -> &LogicalZstValue {
        &self.value
    }
}

impl BoxedNonZeroDescriptor {
    pub const fn descriptor(&self) -> TypeDescriptorRef {
        self.descriptor.reference()
    }

    pub const fn descriptor_exact(&self) -> scoop_identity::PersistentExactTypeId {
        self.descriptor_exact
    }

    pub const fn payload_exact(&self) -> scoop_identity::PersistentExactTypeId {
        self.payload_exact
    }

    pub const fn value(&self) -> &AbiValue {
        &self.value
    }

    pub fn bind_place(
        self,
        locals: &Arena<Local>,
        local: LocalId,
    ) -> Result<BoxValuePlace, BoxDescriptorError> {
        if local.into_raw().into_u32() as usize >= locals.len() {
            return Err(BoxDescriptorError::StorageMismatch);
        }
        if locals[local].storage() != &LocalStorage::NonZero(self.value.clone()) {
            return Err(BoxDescriptorError::StorageMismatch);
        }
        let rooting = match NonEmptyRefScan::new(self.value.scan().clone()) {
            Some(scan) => BoxPayloadRooting::RecursiveRegion(scan),
            None => BoxPayloadRooting::GcFree,
        };
        Ok(BoxValuePlace {
            descriptor: self,
            local,
            rooting,
        })
    }
}

/// A root frame covers this exact local, never a detached copy of its leaves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoxPayloadRooting {
    GcFree,
    RecursiveRegion(NonEmptyRefScan),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxValuePlace {
    descriptor: BoxedNonZeroDescriptor,
    local: LocalId,
    rooting: BoxPayloadRooting,
}

impl BoxValuePlace {
    pub const fn descriptor(&self) -> &BoxedNonZeroDescriptor {
        &self.descriptor
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn rooting(&self) -> &BoxPayloadRooting {
        &self.rooting
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoxPayload {
    ZeroSized(BoxedZstDescriptor),
    NonZero(BoxValuePlace),
}

impl BoxPayload {
    pub fn descriptor(&self) -> TypeDescriptorRef {
        match self {
            Self::ZeroSized(descriptor) => descriptor.descriptor(),
            Self::NonZero(place) => place.descriptor().descriptor(),
        }
    }

    pub fn source(&self) -> Option<LocalId> {
        match self {
            Self::ZeroSized(_) => None,
            Self::NonZero(place) => Some(place.local()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnboxResult {
    ZeroSized {
        descriptor: BoxedZstDescriptor,
        out: TempId,
    },
    NonZero(BoxValuePlace),
}

impl UnboxResult {
    pub fn descriptor(&self) -> TypeDescriptorRef {
        match self {
            Self::ZeroSized { descriptor, .. } => descriptor.descriptor(),
            Self::NonZero(place) => place.descriptor().descriptor(),
        }
    }
}
