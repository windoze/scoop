use super::*;

/// Keeps an external arena reference tied to the selected typed definition.
/// The complete layout selection is checked before this binding is created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoxedDescriptorReference {
    Local(TypeDescriptorId),
    External {
        id: ExternalTypeDescriptorId,
        definition: ExternalTypeDescriptor,
    },
}

impl BoxedDescriptorReference {
    pub(super) const fn reference(self) -> TypeDescriptorRef {
        match self {
            Self::Local(id) => TypeDescriptorRef::Local(id),
            Self::External { id, .. } => TypeDescriptorRef::External(id),
        }
    }
}

impl BoxedValueDescriptor {
    /// Rechecks mutable arena bindings after construction. External layout
    /// facts remain private and originate only from a complete selection.
    pub fn validate_reference(
        &self,
        local: &Arena<TypeDescriptor>,
        external: &Arena<ExternalTypeDescriptor>,
    ) -> Result<(), BoxDescriptorError> {
        let (reference, exact, storage) = match self {
            Self::ZeroSized(value) => (
                value.descriptor,
                value.value.exact(),
                value.value.representation().storage_type(),
            ),
            Self::NonZero(value) => (
                value.descriptor,
                value.payload_exact,
                value.value.storage_type(),
            ),
        };
        match reference {
            BoxedDescriptorReference::Local(id) => {
                if Self::from_local(local, id, exact, storage.clone()).as_ref() != Ok(self) {
                    return Err(BoxDescriptorError::InvalidDescriptor);
                }
            }
            BoxedDescriptorReference::External { id, definition } => {
                if id.into_raw().into_u32() as usize >= external.len() || external[id] != definition
                {
                    return Err(BoxDescriptorError::InvalidDescriptor);
                }
            }
        }
        Ok(())
    }
}
