//! Complete struct fields shared by projection, binding and match lowering.

use super::*;

pub(crate) struct StructField {
    pub(crate) name: String,
    pub(crate) ty: TypeId,
    pub(crate) reference: hir::FieldRef,
}

pub(crate) struct StructFields {
    pub(crate) name: String,
    pub(crate) fields: Vec<StructField>,
}

impl Lowerer {
    /// Resolve declaration-order fields for the complete application. The
    /// consumers use the same field identities and types regardless of where
    /// the declaration is stored.
    pub(crate) fn struct_fields(&mut self, ty: TypeId) -> Option<StructFields> {
        match self.types[ty].clone() {
            Type::Struct(application) => {
                let value = self.struct_applications[application].clone();
                let declaration = &self.structs[value.template];
                let name = declaration.name.clone();
                let fields = declaration.semantic_fields().to_vec();
                let fields = fields
                    .into_iter()
                    .enumerate()
                    .map(|(index, field)| {
                        let reference = self.struct_field_reference(application, index as u32);
                        StructField {
                            name: field.name,
                            ty: self.instantiate_ty(field.ty, &value.arguments),
                            reference,
                        }
                    })
                    .collect();
                Some(StructFields { name, fields })
            }
            Type::ImportedStruct(value) => Some(StructFields {
                name: value.declaration.name().to_owned(),
                fields: value
                    .fields
                    .iter()
                    .map(|field| StructField {
                        name: field.name.clone(),
                        ty: field.ty,
                        reference: hir::FieldRef::StructField {
                            owner: ty,
                            field: field.identity,
                        },
                    })
                    .collect(),
            }),
            _ => None,
        }
    }

    pub(crate) fn struct_field(&mut self, ty: TypeId, name: &str) -> Option<StructField> {
        self.struct_fields(ty)?
            .fields
            .into_iter()
            .find(|field| field.name == name)
    }
}
