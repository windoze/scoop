use super::*;

impl StrongExternalLirBridgeSurfaceV1 {
    pub(crate) fn runtime_string(
        module: &Module,
    ) -> Result<Option<ExternalTypeDescriptor>, StrongExternalLirBridgeBuildError> {
        match module.meta.well_known_type_descriptors.string {
            crate::TypeDescriptorRef::Local(_) => Ok(None),
            crate::TypeDescriptorRef::External(id) => {
                if id.into_raw().into_u32() as usize >= module.meta.external_type_descriptors.len()
                {
                    return Err(StrongExternalLirBridgeBuildError::MissingRuntimeStringDescriptor);
                }
                let descriptor = module.meta.external_type_descriptors[id];
                Self::validate_runtime_string(descriptor)?;
                Ok(Some(descriptor))
            }
        }
    }

    pub(super) fn validate_runtime_string(
        descriptor: ExternalTypeDescriptor,
    ) -> Result<(), StrongExternalLirBridgeBuildError> {
        if descriptor.provider() != scoop_identity::ConeIdentity::CORE {
            return Err(StrongExternalLirBridgeBuildError::InvalidRuntimeStringDescriptor);
        }
        descriptor
            .validate_contract()
            .map_err(StrongExternalLirBridgeBuildError::TypeDescriptor)
    }
}
