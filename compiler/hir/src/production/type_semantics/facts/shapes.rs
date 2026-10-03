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
                let mut variants = Vec::with_capacity(enumeration.variants.len());
                for variant in &enumeration.variants {
                    variants.push(ExactEnumVariantFactsV1 {
                        variant: variant.identity,
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
