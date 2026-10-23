//! Preserve declaration kinds when forming an unresolved lookup key.

use super::*;
use scoop_identity::{
    CallableTemplateOrigin as Callable, DecodedCallableTemplateOrigin as Decoded,
};

impl From<AnnotationCallableV1> for DecodedAnnotationCallableV1 {
    fn from(callable: AnnotationCallableV1) -> Self {
        match callable {
            AnnotationCallableV1::Local(id) => Self::Local(id.into()),
            AnnotationCallableV1::Declaration(declaration) => {
                Self::Declaration(match declaration {
                    Callable::Function(id) => Decoded::Function(id.into()),
                    Callable::GenericFunction(id) => Decoded::GenericFunction(id.into()),
                    Callable::Constructor(id) => Decoded::Constructor(id.into()),
                    Callable::Accessor(id) => Decoded::Accessor(id.into()),
                    Callable::VariantConstructor(id) => Decoded::VariantConstructor(id.into()),
                })
            }
        }
    }
}

impl From<AnnotationTargetV1> for DecodedAnnotationTargetV1 {
    fn from(target: AnnotationTargetV1) -> Self {
        use AnnotationTargetV1 as Target;
        match target {
            Target::Nominal(owner) => Self::Nominal(match owner {
                NominalDeclarationOwner::Concrete(id) => {
                    DecodedNominalDeclarationOwner::Concrete(id.into())
                }
                NominalDeclarationOwner::GenericTemplate(id) => {
                    DecodedNominalDeclarationOwner::GenericTemplate(id.into())
                }
            }),
            Target::Variant(id) => Self::Variant(id.into()),
            Target::Field(id) => Self::Field(id.into()),
            Target::VariantField(id) => Self::VariantField(id.into()),
            Target::Property(id) => Self::Property(id.into()),
            Target::ExtensionProperty(id) => Self::ExtensionProperty(id.into()),
            Target::Annotation(id) => Self::Annotation(id.into()),
            Target::Callable(callable) => Self::Callable(callable.into()),
            Target::Parameter { callable, index } => Self::Parameter {
                callable: callable.into(),
                index,
            },
            Target::CallableReceiver(callable) => Self::CallableReceiver(callable.into()),
            Target::PropertyReceiver(property) => {
                Self::PropertyReceiver(property_reference(property))
            }
            Target::TypeAlias(id) => Self::TypeAlias(id.into()),
            Target::PropertyStorage { property, role } => Self::PropertyStorage {
                property: property_reference(property),
                role,
            },
        }
    }
}

fn property_reference(property: PropertyOwner) -> DecodedPropertyOwner {
    match property {
        PropertyOwner::Property(id) => DecodedPropertyOwner::Property(id.into()),
        PropertyOwner::ExtensionProperty(id) => DecodedPropertyOwner::ExtensionProperty(id.into()),
    }
}
