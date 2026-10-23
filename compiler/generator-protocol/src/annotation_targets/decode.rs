//! Wire references retain their kind until the frontend resolves them.

use super::*;

impl From<PropertyDeclaration> for DecodedPropertyOwner {
    fn from(property: PropertyDeclaration) -> Self {
        match property {
            PropertyDeclaration::Property(id) => Self::Property(id.into()),
            PropertyDeclaration::ExtensionProperty(id) => Self::ExtensionProperty(id.into()),
        }
    }
}

impl From<NominalId> for DecodedNominalDeclarationOwner {
    fn from(nominal: NominalId) -> Self {
        match nominal {
            NominalId::Concrete(id) => Self::Concrete(id.into()),
            NominalId::Generic(id) => Self::GenericTemplate(id.into()),
        }
    }
}

impl From<CallableDeclaration> for hir::DecodedAnnotationCallableV1 {
    fn from(callable: CallableDeclaration) -> Self {
        use CallableDeclaration as Callable;
        use DecodedCallableTemplateOrigin as Declaration;
        Self::Declaration(match callable {
            Callable::Function(id) => Declaration::Function(id.into()),
            Callable::GenericFunction(id) => Declaration::GenericFunction(id.into()),
            Callable::Constructor(id) => Declaration::Constructor(id.into()),
            Callable::PropertyAccessor(id) => Declaration::Accessor(id.into()),
            Callable::VariantConstructor(id) => Declaration::VariantConstructor(id.into()),
            Callable::Generated(id) => return Self::Local(id.into()),
        })
    }
}

impl From<AnnotationTarget> for hir::DecodedAnnotationTargetV1 {
    fn from(target: AnnotationTarget) -> Self {
        match target {
            AnnotationTarget::Nominal { owner } => Self::Nominal(owner.into()),
            AnnotationTarget::Variant { variant } => Self::Variant(variant.into()),
            AnnotationTarget::Field { field } => Self::Field(field.into()),
            AnnotationTarget::VariantField { field } => Self::VariantField(field.into()),
            AnnotationTarget::Property { property } => Self::Property(property.into()),
            AnnotationTarget::ExtensionProperty { property } => {
                Self::ExtensionProperty(property.into())
            }
            AnnotationTarget::Annotation { annotation } => Self::Annotation(annotation.into()),
            AnnotationTarget::Callable { callable } => Self::Callable(callable.into()),
            AnnotationTarget::Parameter { callable, index } => Self::Parameter {
                callable: callable.into(),
                index,
            },
            AnnotationTarget::CallableReceiver { callable } => {
                Self::CallableReceiver(callable.into())
            }
            AnnotationTarget::PropertyReceiver { property } => {
                Self::PropertyReceiver(property.into())
            }
            AnnotationTarget::TypeAlias { alias } => Self::TypeAlias(alias.into()),
            AnnotationTarget::PropertyStorage { property, role } => Self::PropertyStorage {
                property: property.into(),
                role: match role {
                    AnnotationStorageRole::Backing => hir::AnnotationStorageRole::Backing,
                    AnnotationStorageRole::Delegate => hir::AnnotationStorageRole::Delegate,
                },
            },
        }
    }
}
