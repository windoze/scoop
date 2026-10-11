use super::AnnotationTargetV1;
use crate::{
    AnnotationCallableV1, AnnotationDeclarationV1, AnnotationStorageRole, CallableDeclarationId,
    CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1,
    CanonicalTypeAliasInterfacesV1, NominalSourceShapeV1,
};
use scoop_identity::PropertyOwner;
use std::collections::BTreeSet;

impl AnnotationTargetV1 {
    /// Targets are the original declarations retained by the shared source interface.
    pub fn source_targets(
        nominals: &CanonicalNominalInterfacesV1,
        properties: &CanonicalPropertyInterfacesV1,
        callables: &CanonicalCallableInterfacesV1,
        aliases: &CanonicalTypeAliasInterfacesV1,
        annotations: &[AnnotationDeclarationV1],
    ) -> BTreeSet<Self> {
        let mut targets = BTreeSet::new();
        for nominal in nominals.all_records() {
            targets.insert(Self::Nominal(nominal.declaration()));
            targets.extend(
                nominal
                    .source_shape()
                    .declared_fields()
                    .iter()
                    .map(|field| Self::Field(field.field())),
            );
            if let NominalSourceShapeV1::Struct(shape) = nominal.source_shape() {
                targets.extend(
                    shape
                        .fields()
                        .iter()
                        .map(|field| Self::Field(field.field())),
                );
            } else if let NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
                for variant in shape.variants() {
                    targets.insert(Self::Variant(variant.variant()));
                    targets.extend(
                        variant
                            .fields()
                            .iter()
                            .map(|field| Self::VariantField(field.field())),
                    );
                }
            }
        }
        for property in properties.all_declarations() {
            let owner = property.declaration();
            targets.insert(match owner {
                PropertyOwner::Property(id) => Self::Property(id),
                PropertyOwner::ExtensionProperty(id) => Self::ExtensionProperty(id),
            });
            if property.receiver().is_some() {
                targets.insert(Self::PropertyReceiver(owner));
            }
            if property.representation() == crate::PropertyRepresentationV1::RuntimeAccessor {
                for role in [
                    AnnotationStorageRole::Backing,
                    AnnotationStorageRole::Delegate,
                ] {
                    targets.insert(Self::PropertyStorage {
                        property: owner,
                        role,
                    });
                }
            }
            targets.insert(Self::Callable(AnnotationCallableV1::Declaration(
                CallableDeclarationId::Accessor(property.accessors().getter()),
            )));
            if let Some(setter) = property.accessors().setter() {
                let callable =
                    AnnotationCallableV1::Declaration(CallableDeclarationId::Accessor(setter));
                targets.insert(Self::Callable(callable));
                targets.insert(Self::Parameter { callable, index: 0 });
            }
        }
        for callable in callables.all_declarations() {
            let owner = AnnotationCallableV1::Declaration(callable.declaration());
            targets.insert(Self::Callable(owner));
            for index in 0..callable.parameters().len_u32() {
                targets.insert(Self::Parameter {
                    callable: owner,
                    index,
                });
            }
            if callable.receiver().is_some() {
                targets.insert(Self::CallableReceiver(owner));
            }
        }
        targets.extend(
            aliases
                .records()
                .iter()
                .map(|alias| Self::TypeAlias(alias.alias())),
        );
        targets.extend(annotations.iter().map(|a| Self::Annotation(a.annotation)));
        targets
    }

    pub const fn is_lexical_callable(self) -> bool {
        matches!(
            self,
            Self::Callable(AnnotationCallableV1::Local(_))
                | Self::Parameter {
                    callable: AnnotationCallableV1::Local(_),
                    ..
                }
                | Self::CallableReceiver(AnnotationCallableV1::Local(_))
        )
    }
}
