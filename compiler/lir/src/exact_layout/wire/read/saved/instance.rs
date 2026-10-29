use super::*;
use crate::exact_layout::model::InstanceRepresentation;
use crate::{ClassBasePrefixV1, ClassStorageLayoutV1, NonZeroPow2, TypeInstanceShapeV1};

impl RawInstance {
    pub(super) fn read(
        self,
        identity: &ExactLayoutIdentityV1,
        shape: &TypeInstanceShapeV1,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<InstanceRepresentationV1, LinkDataError> {
        Ok(InstanceRepresentationV1(match self {
            Self::Throwable => InstanceRepresentation::Throwable(Box::new(ClassStorageLayoutV1 {
                exact: identity.exact(),
                layout: identity.layout(),
                target: identity.target(),
                base_prefix: ClassBasePrefixV1::NoBase,
                shape: shape.clone(),
                declared_fields: Vec::new(),
                complete_fields: Vec::new(),
            })),
            Self::Atomic(kind) => InstanceRepresentation::Atomic(kind),
            Self::InlineBytes => InstanceRepresentation::InlineBytes,
            Self::AbstractReference => InstanceRepresentation::AbstractReference,
            Self::Box { exact, layout } => {
                let value = layout.read(identity.target(), identities)?;
                if identities.resolve(exact).map_err(link_error)? != value.exact() {
                    return Err(link_error("boxed value layout exact mismatch"));
                }
                InstanceRepresentation::BoxedPayload(value)
            }
            Self::InlineArray { element, storage } => {
                let element = element.read(identity.target(), identities)?;
                InstanceRepresentation::InlineArray {
                    element,
                    storage: storage.validate(identity.target()).map_err(link_error)?,
                }
            }
            Self::Class {
                base,
                declared,
                complete,
            } => {
                let base_prefix = match base {
                    RawBase::NoBase => ClassBasePrefixV1::NoBase,
                    RawBase::Prefix {
                        exact,
                        layout,
                        size,
                        alignment,
                    } => ClassBasePrefixV1::BasePrefix {
                        exact: identities.resolve(exact).map_err(link_error)?,
                        layout: identities.resolve(layout).map_err(link_error)?,
                        byte_size: size,
                        alignment: NonZeroPow2::new(alignment).map_err(link_error)?,
                    },
                };
                InstanceRepresentation::ClassObject(Box::new(ClassStorageLayoutV1 {
                    exact: identity.exact(),
                    layout: identity.layout(),
                    target: identity.target(),
                    base_prefix,
                    shape: shape.clone(),
                    declared_fields: fields(
                        declared,
                        identities,
                        identity.target(),
                        shape.minimum_size(),
                    )?,
                    complete_fields: fields(
                        complete,
                        identities,
                        identity.target(),
                        shape.minimum_size(),
                    )?,
                }))
            }
        }))
    }
}
