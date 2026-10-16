use super::*;

pub(super) enum FactShape {
    Unit,
    Scalar,
    Pointer,
    Reference,
    MaybeUninit {
        value: PersistentExactTypeId,
    },
    OrdinaryStruct {
        fields: Vec<PersistentExactTypeId>,
    },
    CLayoutStruct {
        fields: Vec<PersistentExactTypeId>,
    },
    Tuple {
        elements: Vec<PersistentExactTypeId>,
    },
    Enum {
        fields: Vec<PersistentExactTypeId>,
    },
}

impl FactProjector<'_> {
    pub(super) fn shape(&self, ty: concrete::TypeId) -> Result<FactShape, Error> {
        use concrete::TypeKind;
        let shape = match &self.local.types[ty].kind {
            TypeKind::Unit => FactShape::Unit,
            TypeKind::Integer(_) | TypeKind::Boolean => FactShape::Scalar,
            TypeKind::String
            | TypeKind::Any
            | TypeKind::Class(_)
            | TypeKind::Interface(_)
            | TypeKind::Function(_) => FactShape::Reference,
            TypeKind::Ptr(_) | TypeKind::FunPtr(_) => FactShape::Pointer,
            TypeKind::Tuple(elements) => FactShape::Tuple {
                elements: exacts(self.local, elements)?,
            },
            TypeKind::Struct(id) => {
                let structure = &self.local.structs[*id];
                match &structure.representation {
                    concrete::StructRepresentation::Declared(concrete::DeclaredStruct {
                        attributes,
                        fields,
                        ..
                    }) => {
                        let fields = exacts(
                            self.local,
                            &fields.iter().map(|field| field.ty).collect::<Vec<_>>(),
                        )?;
                        if attributes.c_layout.is_some() {
                            FactShape::CLayoutStruct { fields }
                        } else {
                            FactShape::OrdinaryStruct { fields }
                        }
                    }
                    concrete::StructRepresentation::Intrinsic { application, .. } => {
                        match application {
                            concrete::IntrinsicTypeRepresentation::MaybeUninit { value } => {
                                FactShape::MaybeUninit {
                                    value: exacts(self.local, &[*value])?[0],
                                }
                            }
                            concrete::IntrinsicTypeRepresentation::Unit => FactShape::Unit,
                            concrete::IntrinsicTypeRepresentation::Integer(_)
                            | concrete::IntrinsicTypeRepresentation::Float(_)
                            | concrete::IntrinsicTypeRepresentation::Char
                            | concrete::IntrinsicTypeRepresentation::Boolean => FactShape::Scalar,
                            concrete::IntrinsicTypeRepresentation::Ptr { .. }
                            | concrete::IntrinsicTypeRepresentation::FunPtr { .. } => {
                                FactShape::Pointer
                            }
                            concrete::IntrinsicTypeRepresentation::String
                            | concrete::IntrinsicTypeRepresentation::Atomic(_)
                            | concrete::IntrinsicTypeRepresentation::Any
                            | concrete::IntrinsicTypeRepresentation::Nothing
                            | concrete::IntrinsicTypeRepresentation::Array { .. }
                            | concrete::IntrinsicTypeRepresentation::MutableArray { .. } => {
                                FactShape::Reference
                            }
                        }
                    }
                }
            }
            TypeKind::Enum(id) => {
                let enumeration = &self.local.enums[*id];
                let fields = enumeration
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|field| field.ty))
                    .collect::<Vec<_>>();
                FactShape::Enum {
                    fields: exacts(self.local, &fields)?,
                }
            }
        };
        Ok(shape)
    }
}
