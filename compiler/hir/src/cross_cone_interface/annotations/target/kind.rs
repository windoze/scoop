use super::{AnnotationCallableV1, AnnotationTargetV1};
use crate::{AnnotationTargetKind as Kind, CanonicalPropertyInterfacesV1};
use scoop_identity::CallableTemplateOrigin as Callable;

impl AnnotationTargetV1 {
    pub fn kind(self, properties: &CanonicalPropertyInterfacesV1) -> Option<Kind> {
        Some(match self {
            Self::Nominal(_) => Kind::Class,
            Self::Variant(_) => Kind::EnumVariant,
            Self::Field(_) | Self::VariantField(_) | Self::PropertyStorage { .. } => Kind::Field,
            Self::Property(_) | Self::ExtensionProperty(_) => Kind::Property,
            Self::Annotation(_) => Kind::AnnotationClass,
            Self::Callable(AnnotationCallableV1::Local(_)) => Kind::Function,
            Self::Callable(AnnotationCallableV1::Declaration(declaration)) => match declaration {
                Callable::Function(_) | Callable::GenericFunction(_) => Kind::Function,
                Callable::Constructor(_) | Callable::VariantConstructor(_) => Kind::Constructor,
                Callable::Accessor(id) => {
                    return properties.all_declarations().find_map(|property| {
                        let accessors = property.accessors();
                        if accessors.getter() == id {
                            Some(Kind::PropertyGetter)
                        } else if accessors.setter() == Some(id) {
                            Some(Kind::PropertySetter)
                        } else {
                            None
                        }
                    });
                }
            },
            Self::Parameter { .. } | Self::CallableReceiver(_) | Self::PropertyReceiver(_) => {
                Kind::ValueParameter
            }
            Self::TypeAlias(_) => Kind::TypeAlias,
        })
    }
}
