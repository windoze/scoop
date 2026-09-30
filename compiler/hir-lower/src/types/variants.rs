//! Variant queries retain the original declaration and complete payload types.

use super::*;

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
        match &self.types[owner] {
            Type::Enum(application) => {
                let declaration =
                    &self.enums[self.enum_id(self.enum_applications[*application].template)];
                let index = declaration
                    .variants
                    .iter()
                    .position(|variant| variant.name == name)?;
                Some(self.enum_variant_at(*application, index as u32))
            }
            Type::ImportedEnum(declaration) => {
                let variant = declaration
                    .variants
                    .iter()
                    .find(|variant| variant.name == name)?;
                Some(hir::EnumVariantApplication {
                    owner,
                    variant: variant.identity,
                })
            }
            _ => None,
        }
    }

    pub(crate) fn enum_variant(&mut self, application: hir::EnumVariantApplication) -> EnumVariant {
        let index = self.enum_variant_index(application) as usize;
        match self.types[application.owner].clone() {
            Type::Enum(owner) => {
                let owner = self.enum_applications[owner].clone();
                let declaration = &self.enums[self.enum_id(owner.template)];
                let owner_name = declaration.name.clone();
                let variant = declaration.variants[index].clone();
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
            Type::ImportedEnum(owner) => {
                let variant = &owner.variants[index];
                EnumVariant {
                    application,
                    owner_name: owner.declaration.name().to_owned(),
                    name: variant.name.clone(),
                    style: match variant.style {
                        hir::EnumSourceVariantStyleV1::Unit => hir::VariantStyle::Unit,
                        hir::EnumSourceVariantStyleV1::Positional => hir::VariantStyle::Positional,
                        hir::EnumSourceVariantStyleV1::Named => hir::VariantStyle::Named,
                        hir::EnumSourceVariantStyleV1::Constructor => {
                            hir::VariantStyle::Constructor
                        }
                    },
                    fields: variant
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), field.ty))
                        .collect(),
                }
            }
            _ => unreachable!("a variant query retains its complete enum owner"),
        }
    }

    pub(crate) fn enum_variants(&mut self, owner: TypeId) -> Vec<EnumVariant> {
        let applications = match &self.types[owner] {
            Type::Enum(application) => {
                let declaration =
                    &self.enums[self.enum_id(self.enum_applications[*application].template)];
                (0..declaration.variants.len())
                    .map(|index| self.enum_variant_at(*application, index as u32))
                    .collect::<Vec<_>>()
            }
            Type::ImportedEnum(declaration) => declaration
                .variants
                .iter()
                .map(|variant| hir::EnumVariantApplication {
                    owner,
                    variant: variant.identity,
                })
                .collect(),
            _ => unreachable!("variant enumeration requires an enum application"),
        };
        applications
            .into_iter()
            .map(|application| self.enum_variant(application))
            .collect()
    }
}
