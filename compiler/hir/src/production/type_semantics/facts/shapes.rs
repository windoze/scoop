use super::*;

impl FactProjector<'_> {
    pub(super) fn shape(&self, ty: concrete::TypeId) -> Result<ExactTypeFactShapeV1, Error> {
        use concrete::TypeKind;
        let shape = match &self.local.types[ty].kind {
            TypeKind::Unit => ExactTypeFactShapeV1::Unit,
            TypeKind::Integer(_) | TypeKind::Boolean => ExactTypeFactShapeV1::Scalar,
            TypeKind::String
            | TypeKind::Any
            | TypeKind::Class(_)
            | TypeKind::Interface(_)
            | TypeKind::Function(_) => ExactTypeFactShapeV1::Reference,
            TypeKind::Ptr(_) | TypeKind::FunPtr(_) => ExactTypeFactShapeV1::Pointer,
            TypeKind::Tuple(elements) => ExactTypeFactShapeV1::Tuple {
                elements: exacts(self.local, elements)?,
            },
            TypeKind::Struct(id) => {
                let structure = &self.local.structs[*id];
                match &structure.representation {
                    concrete::StructRepresentation::Declared {
                        attributes, fields, ..
                    } => {
                        let fields = exacts(
                            self.local,
                            &fields.iter().map(|field| field.ty).collect::<Vec<_>>(),
                        )?;
                        if attributes.c_layout.is_some() {
                            ExactTypeFactShapeV1::CLayoutStruct { fields }
                        } else {
                            ExactTypeFactShapeV1::OrdinaryStruct { fields }
                        }
                    }
                    concrete::StructRepresentation::Intrinsic { application, .. } => {
                        match application {
                            concrete::IntrinsicTypeRepresentation::Integer(_)
                            | concrete::IntrinsicTypeRepresentation::Boolean => {
                                ExactTypeFactShapeV1::Scalar
                            }
                            concrete::IntrinsicTypeRepresentation::Ptr { .. }
                            | concrete::IntrinsicTypeRepresentation::FunPtr { .. } => {
                                ExactTypeFactShapeV1::Pointer
                            }
                            concrete::IntrinsicTypeRepresentation::String
                            | concrete::IntrinsicTypeRepresentation::Array { .. }
                            | concrete::IntrinsicTypeRepresentation::MutableArray { .. } => {
                                ExactTypeFactShapeV1::Reference
                            }
                        }
                    }
                }
            }
            TypeKind::Enum(id) => {
                let enumeration = &self.local.enums[*id];
                let exact = self.exact(ty)?;
                let owner = enumeration
                    .origin
                    .source()
                    .map(super::super::nominals::source_id)
                    .ok_or(Error::MissingExactIdentity)?;
                let source_enum = self
                    .export
                    .enums
                    .iter()
                    .find_map(|(id, _)| {
                        (self.export.nominal_identities[id]
                            .source()
                            .map(super::super::nominals::source_id)
                            == Some(owner))
                        .then_some(id)
                    })
                    .ok_or(Error::MissingConcreteType(exact))?;
                let mut variants = Vec::with_capacity(enumeration.variants.len());
                for (index, variant) in enumeration.variants.iter().enumerate() {
                    let index = u32::try_from(index).map_err(|_| Error::InvalidFact {
                        exact,
                        reason: "enum variant count exceeds typed identity index".into(),
                    })?;
                    let reference = EnumVariantRef::checked(&self.export.enums, source_enum, index)
                        .ok_or_else(|| Error::InvalidFact {
                            exact,
                            reason: format!("missing enum variant identity at index {index}"),
                        })?;
                    let variant_id = self.export.enum_member_identities[reference].id();
                    variants.push(ExactEnumVariantFactsV1 {
                        variant: variant_id,
                        fields: exacts(
                            self.local,
                            &variant
                                .fields
                                .iter()
                                .map(|field| field.ty)
                                .collect::<Vec<_>>(),
                        )?,
                        gc: if variant.gc_free {
                            ExactTypeGcV1::GcFree
                        } else {
                            ExactTypeGcV1::ContainsManagedReferences
                        },
                    });
                }
                ExactTypeFactShapeV1::Enum { variants }
            }
        };
        Ok(shape)
    }
}
