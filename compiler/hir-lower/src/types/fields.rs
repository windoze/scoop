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
    pub(crate) fn struct_c_layout(
        &self,
        template: hir::SourceNominalId,
    ) -> Option<hir::HirCLayoutContract> {
        self.struct_definition(template).attributes.c_layout
    }

    pub(crate) fn struct_interior_mutable(&self, template: hir::SourceNominalId) -> bool {
        self.struct_definition(template).attributes.interior_mutable
    }

    /// Resolve declaration-order fields for the complete application. The
    /// consumers use the same field identities and types regardless of where
    /// the declaration is stored.
    pub(crate) fn struct_fields(&mut self, ty: TypeId) -> Option<StructFields> {
        match self.types[ty].clone() {
            Type::Struct(application) => {
                let value = self.struct_applications[application].clone();
                let declaration = self.struct_definition(value.template);
                let name = self.nominal_template_name(value.template).to_owned();
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
