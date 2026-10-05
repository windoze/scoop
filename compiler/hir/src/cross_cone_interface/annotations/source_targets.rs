use super::AnnotationTargetV1;
use crate::{
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, NominalSourceShapeV1,
    PublicDeclarationOwnerV1, PublicNominalKindV1,
};
use scoop_identity::PropertyOwner;
use std::collections::BTreeSet;

impl AnnotationTargetV1 {
    /// Targets are the original declarations retained by the shared source interface.
    pub fn source_targets(
        nominals: &CanonicalNominalInterfacesV1,
        properties: &CanonicalPropertyInterfacesV1,
    ) -> BTreeSet<Self> {
        let mut targets = BTreeSet::new();
        for nominal in nominals.all_records() {
            targets.insert(Self::Nominal(nominal.declaration()));
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
            let (PropertyOwner::Property(id), PublicDeclarationOwnerV1::Nominal(owner)) =
                (property.declaration(), property.owner())
            else {
                continue;
            };
            if nominals.declaration(owner).is_some_and(|nominal| {
                matches!(
                    nominal.kind(),
                    PublicNominalKindV1::Class
                        | PublicNominalKindV1::Interface
                        | PublicNominalKindV1::Object
                )
            }) {
                targets.insert(Self::Property(id));
            }
        }
        targets
    }
}
