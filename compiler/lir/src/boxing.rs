//! Descriptor-refined boxing and unboxing operations.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxedZstDescriptor {
    descriptor: TypeDescriptorId,
    descriptor_exact: scoop_identity::PersistentExactTypeId,
    value: LogicalZstValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxedNonZeroDescriptor {
    descriptor: TypeDescriptorId,
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

impl BoxedValueDescriptor {
    pub fn from_local(
        descriptors: &Arena<TypeDescriptor>,
        descriptor: TypeDescriptorId,
        payload_exact: scoop_identity::PersistentExactTypeId,
        storage_type: LirType,
    ) -> Result<Self, BoxDescriptorError> {
        if descriptor.into_raw().into_u32() as usize >= descriptors.len() {
            return Err(BoxDescriptorError::InvalidDescriptor);
        }
        let definition = &descriptors[descriptor];
        let shape = &definition.instance_shape;
        if shape.instance_kind() != TypeInstanceKindV1::BoxedValue {
            return Err(BoxDescriptorError::NotBoxedValue);
        }
        let descriptor_exact = definition.identity.exact_type();
        let nominal = scoop_identity::PersistentTypeId::from_generated_key(
            &scoop_identity::GeneratedNominalKey::BoxedValue {
                payload: payload_exact,
            },
        )
        .map_err(|_| BoxDescriptorError::PayloadIdentityMismatch)?;
        let expected = scoop_identity::PersistentExactTypeId::from_key(
            &scoop_identity::ExactTypeKey::Nominal(nominal),
        )
        .map_err(|_| BoxDescriptorError::PayloadIdentityMismatch)?;
        if descriptor_exact != expected {
            return Err(BoxDescriptorError::PayloadIdentityMismatch);
        }

        match shape.inline_storage_kind() {
            InlineStorageKindV1::ZeroSized => {
                let layout = AbiZeroSizedLayout::new(shape.inline_alignment())
                    .map_err(|_| BoxDescriptorError::InvalidValueLayout)?;
                let representation = AbiZst::new(storage_type, layout)
                    .map_err(|_| BoxDescriptorError::InvalidValueLayout)?;
                Ok(Self::ZeroSized(BoxedZstDescriptor {
                    descriptor,
                    descriptor_exact,
                    value: LogicalZstValue::new(payload_exact, representation),
                }))
            }
            InlineStorageKindV1::Inline => {
                let layout = AbiNonZeroLayout::new(shape.inline_size(), shape.inline_alignment())
                    .map_err(|_| BoxDescriptorError::InvalidValueLayout)?;
                let value = AbiValue::new(storage_type, layout, shape.inline_scan().clone())
                    .map_err(|_| BoxDescriptorError::InvalidValueLayout)?;
                Ok(Self::NonZero(BoxedNonZeroDescriptor {
                    descriptor,
                    descriptor_exact,
                    payload_exact,
                    value,
                }))
            }
            InlineStorageKindV1::None => Err(BoxDescriptorError::NotBoxedValue),
        }
    }
}

impl BoxedZstDescriptor {
    pub const fn descriptor(&self) -> TypeDescriptorId {
        self.descriptor
    }

    pub const fn descriptor_exact(&self) -> scoop_identity::PersistentExactTypeId {
        self.descriptor_exact
    }

    pub const fn value(&self) -> &LogicalZstValue {
        &self.value
    }
}

impl BoxedNonZeroDescriptor {
    pub const fn descriptor(&self) -> TypeDescriptorId {
        self.descriptor
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
    pub fn descriptor(&self) -> TypeDescriptorId {
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
    pub fn descriptor(&self) -> TypeDescriptorId {
        match self {
            Self::ZeroSized { descriptor, .. } => descriptor.descriptor(),
            Self::NonZero(place) => place.descriptor().descriptor(),
        }
    }
}
