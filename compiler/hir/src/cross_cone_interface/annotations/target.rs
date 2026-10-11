use super::wire::AnnotationDataResolver;
use scoop_identity::{
    DecodedNominalDeclarationOwner, DecodedPersistentId, DecodedPropertyOwner,
    NominalDeclarationOwner, PersistentAnnotationId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentPropertyId, PersistentTypeAliasId,
    PropertyOwner,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

mod callable;
mod wire;
pub use callable::AnnotationCallableV1;
use callable::DecodedAnnotationCallableV1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AnnotationTargetV1 {
    Nominal(NominalDeclarationOwner),
    Variant(PersistentEnumVariantId),
    Field(PersistentFieldId),
    VariantField(PersistentEnumVariantFieldId),
    Property(PersistentPropertyId),
    ExtensionProperty(scoop_identity::PersistentExtensionPropertyId),
    Annotation(PersistentAnnotationId),
    Callable(AnnotationCallableV1),
    Parameter {
        callable: AnnotationCallableV1,
        index: u32,
    },
    CallableReceiver(AnnotationCallableV1),
    PropertyReceiver(PropertyOwner),
    TypeAlias(PersistentTypeAliasId),
    PropertyStorage {
        property: PropertyOwner,
        role: crate::AnnotationStorageRole,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedAnnotationTargetV1 {
    Nominal(DecodedNominalDeclarationOwner),
    Variant(DecodedPersistentId<PersistentEnumVariantId>),
    Field(DecodedPersistentId<PersistentFieldId>),
    VariantField(DecodedPersistentId<PersistentEnumVariantFieldId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
    ExtensionProperty(DecodedPersistentId<scoop_identity::PersistentExtensionPropertyId>),
    Annotation(DecodedPersistentId<PersistentAnnotationId>),
    Callable(DecodedAnnotationCallableV1),
    Parameter {
        callable: DecodedAnnotationCallableV1,
        index: u32,
    },
    CallableReceiver(DecodedAnnotationCallableV1),
    PropertyReceiver(DecodedPropertyOwner),
    TypeAlias(DecodedPersistentId<PersistentTypeAliasId>),
    PropertyStorage {
        property: DecodedPropertyOwner,
        role: crate::AnnotationStorageRole,
    },
}

impl DecodedAnnotationTargetV1 {
    pub fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<AnnotationTargetV1, E> {
        match self {
            Self::Nominal(id) => id.resolve(resolver).map(AnnotationTargetV1::Nominal),
            Self::Variant(id) => resolver.resolve(id).map(AnnotationTargetV1::Variant),
            Self::Field(id) => resolver.resolve(id).map(AnnotationTargetV1::Field),
            Self::VariantField(id) => resolver.resolve(id).map(AnnotationTargetV1::VariantField),
            Self::Property(id) => resolver.resolve(id).map(AnnotationTargetV1::Property),
            Self::ExtensionProperty(id) => resolver
                .resolve(id)
                .map(AnnotationTargetV1::ExtensionProperty),
            Self::Annotation(id) => resolver.resolve(id).map(AnnotationTargetV1::Annotation),
            Self::Callable(id) => id.resolve(resolver).map(AnnotationTargetV1::Callable),
            Self::Parameter { callable, index } => Ok(AnnotationTargetV1::Parameter {
                callable: callable.resolve(resolver)?,
                index,
            }),
            Self::CallableReceiver(id) => id
                .resolve(resolver)
                .map(AnnotationTargetV1::CallableReceiver),
            Self::PropertyReceiver(id) => id
                .resolve(resolver)
                .map(AnnotationTargetV1::PropertyReceiver),
            Self::TypeAlias(id) => resolver.resolve(id).map(AnnotationTargetV1::TypeAlias),
            Self::PropertyStorage { property, role } => Ok(AnnotationTargetV1::PropertyStorage {
                property: property.resolve(resolver)?,
                role,
            }),
        }
    }
}
