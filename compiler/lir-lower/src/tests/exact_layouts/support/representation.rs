use super::*;

pub(super) fn representation(
    module: &mir::Module,
    ty: &mir::Type,
    owner: &SourceDeclarationKey,
    pending: &mut PendingIdentityValidation<'_>,
) -> (
    mir::MirTypeRepresentationV1,
    mir::MirTypeFactsV1,
    mir::MirBaseClassV1,
) {
    use mir::{
        MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Repr,
        MirValueKindV1 as Kind,
    };
    let mut base = mir::MirBaseClassV1::None;
    let (representation, kind, gc) = match ty {
        mir::Type::Unit => (Repr::Intrinsic(Intrinsic::Unit), Kind::ZeroSizedValue, true),
        mir::Type::Integer(kind) => (
            Repr::Intrinsic(Intrinsic::Integer(*kind)),
            Kind::NonZeroValue,
            true,
        ),
        mir::Type::Boolean => (
            Repr::Intrinsic(Intrinsic::Boolean),
            Kind::NonZeroValue,
            true,
        ),
        mir::Type::String => (Repr::Intrinsic(Intrinsic::String), Kind::Reference, false),
        mir::Type::Struct(id) => {
            let definition = &module.structs[*id];
            let mir::StructRepresentation::Declared {
                fields,
                c_layout,
                interior_mutable,
            } = &definition.representation
            else {
                panic!("declared")
            };
            let empty = fields.iter().all(|field| field.ty == mir::Type::Unit);
            let fields = fields
                .iter()
                .map(|field| {
                    let identity = CborIdentityRecord::from_key(
                        FieldIdentityKey::source_declared(
                            owner,
                            CanonicalIdentifier::new(&field.name).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    let field_id = identity.id();
                    pending
                        .register_external_canonical_authority(identity)
                        .unwrap();
                    mir::MirRepresentationFieldV1 {
                        field: field_id,
                        value: exact(module, &field.ty),
                    }
                })
                .collect();
            (
                Repr::Struct {
                    fields,
                    c_layout: c_layout.map_or(
                        mir::MirTypeCLayoutPolicyV1::Ordinary,
                        mir::MirTypeCLayoutPolicyV1::CLayout,
                    ),
                    interior_mutable: *interior_mutable,
                },
                if empty {
                    Kind::ZeroSizedValue
                } else {
                    Kind::NonZeroValue
                },
                definition.gc_free,
            )
        }
        mir::Type::Class(id) => {
            let definition = &module.classes[*id];
            let mir::ClassRepresentation::Declared { fields, base_class } =
                &definition.representation
            else {
                panic!("declared")
            };
            let inherited = base_class.map_or(0, |id| {
                base = mir::MirBaseClassV1::Base(exact(module, &mir::Type::Class(id)));
                module.classes[id].declared_fields().len()
            });
            let fields = fields[inherited..]
                .iter()
                .map(|field| {
                    let property = PersistentPropertyId::from_source_declaration(
                        &SourceDeclarationKey::property(
                            site(),
                            CanonicalIdentifier::new(&field.name).unwrap(),
                        ),
                    )
                    .unwrap();
                    let identity = CborIdentityRecord::from_key(
                        FieldIdentityKey::source_property_backing(owner, property).unwrap(),
                    )
                    .unwrap();
                    let field_id = identity.id();
                    pending
                        .register_external_canonical_authority(identity)
                        .unwrap();
                    mir::MirRepresentationFieldV1 {
                        field: field_id,
                        value: exact(module, &field.ty),
                    }
                })
                .collect();
            let kind = match definition.modifier {
                mir::ClassModifier::Final => mir::MirClassKindV1::Final,
                mir::ClassModifier::Open => mir::MirClassKindV1::Open,
                mir::ClassModifier::Abstract => mir::MirClassKindV1::Abstract,
            };
            (
                Repr::Class {
                    kind,
                    declared_fields: fields,
                },
                Kind::Reference,
                false,
            )
        }
        mir::Type::Interface(_) => (Repr::Interface, Kind::Reference, false),
        mir::Type::Enum(id, _) => {
            let definition = &module.enums[*id];
            let variants = definition
                .variants
                .iter()
                .map(|variant| {
                    let key = CborIdentityRecord::from_key(
                        EnumVariantIdentityKey::source(
                            owner,
                            CanonicalIdentifier::new(&variant.name).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    let variant_id = key.id();
                    pending.register_external_canonical_authority(key).unwrap();
                    let fields = variant
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            let key = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
                                variant_id,
                                EnumVariantFieldSelector::Positional {
                                    declaration_index: index as u32,
                                },
                            ))
                            .unwrap();
                            let field_id = key.id();
                            pending.register_external_canonical_authority(key).unwrap();
                            mir::MirRepresentationVariantFieldV1 {
                                field: field_id,
                                value: exact(module, &field.ty),
                            }
                        })
                        .collect();
                    mir::MirRepresentationVariantV1 {
                        variant: variant_id,
                        fields,
                        gc: if variant.gc_free {
                            mir::MirGcKindV1::GcFree
                        } else {
                            mir::MirGcKindV1::ContainsManagedReferences
                        },
                    }
                })
                .collect();
            (
                Repr::Enum { variants },
                Kind::NonZeroValue,
                definition.gc_free,
            )
        }
        _ => panic!("finite fixture type"),
    };
    (
        representation,
        mir::MirTypeFactsV1::try_new(
            kind,
            if gc {
                mir::MirGcKindV1::GcFree
            } else {
                mir::MirGcKindV1::ContainsManagedReferences
            },
        )
        .unwrap(),
        base,
    )
}
