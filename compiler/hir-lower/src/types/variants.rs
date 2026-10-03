//! Variant queries retain the original declaration and complete payload types.

use super::*;

mod definition;

pub(crate) struct EnumVariant {
    pub(crate) application: hir::EnumVariantApplication,
    pub(crate) owner_name: String,
    pub(crate) name: String,
    pub(crate) style: hir::VariantStyle,
    pub(crate) fields: Vec<(String, TypeId)>,
}

impl Lowerer {
    pub(crate) fn named_enum_variant(
        &self,
        owner: TypeId,
        name: &str,
    ) -> Option<hir::EnumVariantApplication> {
        let Type::Enum(application) = self.types[owner] else {
            return None;
        };
        let definition = self.enum_definition(self.enum_applications[application].template);
        let index = definition
            .variants
            .iter()
            .position(|variant| variant.name == name)?;
        Some(self.enum_variant_at(application, index as u32))
    }

    pub(crate) fn enum_variant(&mut self, application: hir::EnumVariantApplication) -> EnumVariant {
        let index = self.enum_variant_index(application) as usize;
        let Type::Enum(owner) = self.types[application.owner] else {
            unreachable!("a variant query retains its complete enum owner")
        };
        let owner = self.enum_applications[owner].clone();
        let owner_name = self.nominal_template_name(owner.template).to_owned();
        let variant = self.enum_definition(owner.template).variants[index].clone();
        let fields = variant
            .fields
            .into_iter()
            .map(|field| (field.name, self.instantiate_ty(field.ty, &owner.arguments)))
            .collect();
        EnumVariant {
            application,
            owner_name,
            name: variant.name,
            style: variant.style,
            fields,
        }
    }

    pub(crate) fn enum_variants(&mut self, owner: TypeId) -> Vec<EnumVariant> {
        let Type::Enum(application) = self.types[owner] else {
            unreachable!("variant enumeration requires an enum application")
        };
        let count = self
            .enum_definition(self.enum_applications[application].template)
            .variants
            .len();
        (0..count)
            .map(|index| {
                let application = self.enum_variant_at(application, index as u32);
                self.enum_variant(application)
            })
            .collect()
    }
}
