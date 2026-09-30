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
        if let Some(id) = self.source_struct_id(template) {
            return self.structs[id].attributes.c_layout;
        }
        let hir::NominalSourceShapeV1::Struct(shape) = self.loaded_struct_definitions[&template]
            .declaration
            .interface
            .source_shape()
        else {
            unreachable!("a struct definition retains its declaration shape")
        };
        match shape.c_layout_policy() {
            hir::NominalCLayoutPolicyV1::Ordinary => None,
            hir::NominalCLayoutPolicyV1::CLayout { contract } => Some(contract),
        }
    }

    pub(crate) fn struct_interior_mutable(&self, template: hir::SourceNominalId) -> bool {
        if let Some(id) = self.source_struct_id(template) {
            return self.structs[id].attributes.interior_mutable;
        }
        let hir::NominalSourceShapeV1::Struct(shape) = self.loaded_struct_definitions[&template]
            .declaration
            .interface
            .source_shape()
        else {
            unreachable!("a struct definition retains its declaration shape")
        };
        shape.interior_mutable()
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
