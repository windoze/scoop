use super::*;

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
        Self::from_shape(
            BoxedDescriptorReference::Local(descriptor),
            definition.identity.exact_type(),
            &definition.instance_shape,
            payload_exact,
            storage_type,
        )
    }

    pub(crate) fn from_shape(
        descriptor: BoxedDescriptorReference,
        descriptor_exact: scoop_identity::PersistentExactTypeId,
        shape: &TypeInstanceShapeV1,
        payload_exact: scoop_identity::PersistentExactTypeId,
        storage_type: LirType,
    ) -> Result<Self, BoxDescriptorError> {
        if shape.instance_kind() != TypeInstanceKindV1::BoxedValue {
            return Err(BoxDescriptorError::NotBoxedValue);
        }
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
