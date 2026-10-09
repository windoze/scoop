use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn establish_enum_member_identities(
        &mut self,
    ) -> Result<(), PersistentEnumMemberIdentityError> {
        let nominals = self
            .nominal_identities
            .as_ref()
            .expect("nominal declarations precede their variants");
        let identities = hir::HirEnumMemberIdentities::from_declarations(&self.enums, nominals)
            .map_err(|error| {
                let enumeration = hir::EnumId::from_raw(error.enumeration().into());
                PersistentEnumMemberIdentityError {
                    file: self.enum_files.get(&enumeration).copied().unwrap_or(0),
                    span: self.enums[enumeration].span,
                    error,
                }
            })?;
        self.enum_member_identities = Some(identities);
        Ok(())
    }

    pub(crate) fn enum_variant_reference(
        &self,
        variant: hir::AppliedEnumVariantRef,
    ) -> hir::EnumVariantApplication {
        hir::EnumVariantApplication {
            owner: self.enum_applications[variant.application()].canonical_type,
            variant: self
                .enum_member_identities
                .as_ref()
                .expect("variant identities precede bodies")[variant.declaration()]
            .id(),
        }
    }

    pub(crate) fn enum_variant_at(
        &self,
        application: hir::EnumApplicationId,
        index: u32,
    ) -> hir::EnumVariantApplication {
        let owner = &self.enum_applications[application];
        if let Some(definition) = self.loaded_enum_definitions.get(&owner.template) {
            return hir::EnumVariantApplication {
                owner: owner.canonical_type,
                variant: definition.variant_identity(index as usize),
            };
        }
        let reference = hir::AppliedEnumVariantRef::checked_index(
            &self.enums,
            &self.enum_applications,
            self.nominal_identities
                .as_ref()
                .expect("nominal identities precede application references"),
            application,
            index,
        )
        .expect("the variant belongs to its declaring application");
        self.enum_variant_reference(reference)
    }

    pub(crate) fn enum_variant_index(&self, application: hir::EnumVariantApplication) -> u32 {
        let hir::Type::Enum(owner) = self.types[application.owner] else {
            unreachable!("an enum variant retains its complete enum owner")
        };
        let owner = self.enum_applications[owner].template;
        if let Some(definition) = self.loaded_enum_definitions.get(&owner) {
            return definition
                .variant_index(application.variant)
                .expect("a variant belongs to its declaring enum") as u32;
        }
        self.enum_member_identities
            .as_ref()
            .expect("variant identities precede patterns")
            .variant_declaration(application.variant)
            .expect("a current variant retains its declaration identity")
            .local_index()
    }

    pub(crate) fn enum_variant_field_at(
        &self,
        variant: hir::EnumVariantApplication,
        index: u32,
    ) -> hir::EnumVariantFieldApplication {
        let hir::Type::Enum(owner) = self.types[variant.owner] else {
            unreachable!("a variant field retains its enum owner")
        };
        let template = self.enum_applications[owner].template;
        let field = if let Some(definition) = self.loaded_enum_definitions.get(&template) {
            definition.field_identity(self.enum_variant_index(variant) as usize, index as usize)
        } else {
            let identities = self
                .enum_member_identities
                .as_ref()
                .expect("enum members are identified");
            let declaration = identities
                .variant_declaration(variant.variant)
                .expect("the variant belongs to its declaration");
            let field = hir::EnumVariantFieldRef::checked(&self.enums, declaration, index)
                .expect("the field belongs to its variant");
            identities[field].id()
        };
        hir::EnumVariantFieldApplication { variant, field }
    }
}

#[derive(Debug)]
pub(crate) struct PersistentEnumMemberIdentityError {
    file: usize,
    span: Span,
    error: hir::HirEnumMemberIdentityError,
}

impl PersistentEnumMemberIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentEnumMemberIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent enum-member identity: {}",
            self.error
        )
    }
}

impl std::error::Error for PersistentEnumMemberIdentityError {}

#[cfg(test)]
mod tests;
