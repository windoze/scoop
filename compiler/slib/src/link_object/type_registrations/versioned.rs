use scoop_identity::{ConeIdentity, PersistentCallableBodyId, PersistentExactTypeId};
use scoop_lir::{
    RuntimeFunction, StrongDescriptorReference, StrongTypeDescriptorRefV1,
    StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV1, StrongTypeDispatchCallableRefV2,
};
use scoop_wire::{RuntimeEncodeError, RuntimeEncoder};

use crate::link_object::callable_registrations::object_definition::CanonicalObjectRelocationV1;

#[derive(Clone, Copy)]
pub(in crate::link_object) enum DescriptorReferenceKind {
    Local(PersistentExactTypeId),
    External(PersistentExactTypeId),
}

pub(in crate::link_object) trait LinkDescriptorReference:
    StrongDescriptorReference
{
    fn kind(self) -> DescriptorReferenceKind;

    fn canonical_relocation(self, offset: u64) -> CanonicalObjectRelocationV1;

    fn runtime_encode(self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError>;
}

impl LinkDescriptorReference for StrongTypeDescriptorRefV1 {
    fn kind(self) -> DescriptorReferenceKind {
        current_descriptor(self).kind()
    }

    fn canonical_relocation(self, offset: u64) -> CanonicalObjectRelocationV1 {
        current_descriptor(self).canonical_relocation(offset)
    }

    fn runtime_encode(self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        current_descriptor(self).runtime_encode(encoder)
    }
}

fn current_descriptor(reference: StrongTypeDescriptorRefV1) -> StrongTypeDescriptorRefV2 {
    match reference {
        StrongTypeDescriptorRefV1::Local(exact) => StrongTypeDescriptorRefV2::Local(exact),
        StrongTypeDescriptorRefV1::DependencyExternal { provider, exact } => {
            StrongTypeDescriptorRefV2::DependencyExternal { provider, exact }
        }
    }
}

impl LinkDescriptorReference for StrongTypeDescriptorRefV2 {
    fn kind(self) -> DescriptorReferenceKind {
        match self {
            Self::Local(exact) => DescriptorReferenceKind::Local(exact),
            Self::DependencyExternal { exact, .. } => DescriptorReferenceKind::External(exact),
        }
    }

    fn canonical_relocation(self, offset: u64) -> CanonicalObjectRelocationV1 {
        match self {
            Self::Local(exact) => {
                CanonicalObjectRelocationV1::intra_cone_type_descriptor(offset, exact)
            }
            Self::DependencyExternal { provider, exact } => {
                CanonicalObjectRelocationV1::dependency_target(
                    offset,
                    provider,
                    scoop_lir::ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
                )
            }
        }
    }

    fn runtime_encode(self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self {
            Self::Local(exact) => {
                encoder.u32(1)?;
                encoder.fixed(exact.as_array())
            }
            Self::DependencyExternal { provider, exact } => {
                encoder.u32(3)?;
                encoder.fixed(provider.as_array())?;
                encoder.fixed(exact.as_array())
            }
        }
    }
}

pub(in crate::link_object) trait LinkDispatchCallableReference:
    Copy
{
    fn runtime_encode(self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError>;
}

impl LinkDispatchCallableReference for StrongTypeDispatchCallableRefV1 {
    fn runtime_encode(self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encode_dispatch(
            encoder,
            match self {
                Self::Local(body) => DispatchReferenceKind::Local(body),
                Self::Runtime(function) => DispatchReferenceKind::Runtime(function),
                Self::DependencyExternal { provider, body } => {
                    DispatchReferenceKind::DependencyExternal { provider, body }
                }
            },
        )
    }
}

impl LinkDispatchCallableReference for StrongTypeDispatchCallableRefV2 {
    fn runtime_encode(self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encode_dispatch(
            encoder,
            match self {
                Self::Local(body) => DispatchReferenceKind::Local(body),
                Self::Runtime(function) => DispatchReferenceKind::Runtime(function),
                Self::DependencyExternal { provider, body } => {
                    DispatchReferenceKind::DependencyExternal { provider, body }
                }
            },
        )
    }
}

enum DispatchReferenceKind {
    Local(PersistentCallableBodyId),
    Runtime(RuntimeFunction),
    DependencyExternal {
        provider: ConeIdentity,
        body: PersistentCallableBodyId,
    },
}

fn encode_dispatch(
    encoder: &mut RuntimeEncoder,
    reference: DispatchReferenceKind,
) -> Result<(), RuntimeEncodeError> {
    match reference {
        DispatchReferenceKind::Local(body) => {
            encoder.u32(1)?;
            encoder.fixed(body.as_array())
        }
        DispatchReferenceKind::Runtime(function) => {
            encoder.u32(3)?;
            encoder.u64(function.wire_family_tag())?;
            encoder.u64(function.wire_function_tag())
        }
        DispatchReferenceKind::DependencyExternal { provider, body } => {
            encoder.u32(4)?;
            encoder.fixed(provider.as_array())?;
            encoder.fixed(body.as_array())
        }
    }
}

#[cfg(test)]
mod tests;
