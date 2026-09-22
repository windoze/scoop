use super::*;

/// The descriptor role is explicit and independent of callable selections.
#[derive(Clone, Copy, Debug)]
pub enum RuntimeStringDescriptor {
    Local,
    External(lir::ExternalTypeDescriptor),
}

pub(super) fn lower_runtime_string(
    input: &mir::SingleConeStrongMirInput,
    descriptor: RuntimeStringDescriptor,
) -> Result<
    (
        Arena<lir::ExternalTypeDescriptor>,
        Option<lir::TypeDescriptorRef>,
    ),
    StrongLirLoweringError,
> {
    let producer = input.module().cone;
    match descriptor {
        RuntimeStringDescriptor::Local => Ok((Arena::new(), None)),
        RuntimeStringDescriptor::External(descriptor) => {
            if descriptor.provider() == producer {
                return Err(StrongLirLoweringError::RuntimeStringDescriptorOwnership { producer });
            }
            if let Some(identity) = input
                .module()
                .meta
                .source_exact_types
                .get(&mir::Type::String)
                && identity.identity_record().id() != descriptor.target()
            {
                return Err(StrongLirLoweringError::RuntimeStringExactMismatch {
                    mir: identity.identity_record().id(),
                    lir: descriptor.target(),
                });
            }
            let mut descriptors = Arena::new();
            let reference = lir::TypeDescriptorRef::External(descriptors.alloc(descriptor));
            Ok((descriptors, Some(reference)))
        }
    }
}
