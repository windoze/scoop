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
        let reference = hir::AppliedEnumVariantRef::checked_index(
            &self.enums,
            &self.enum_applications,
            application,
            index,
        )
        .expect("the variant belongs to its declaring application");
        self.enum_variant_reference(reference)
    }

    pub(crate) fn enum_variant_index(&self, application: hir::EnumVariantApplication) -> u32 {
        match &self.types[application.owner] {
            hir::Type::Enum(_) => self
                .enum_member_identities
                .as_ref()
                .expect("variant identities precede patterns")
                .variant_declaration(application.variant)
                .expect("a current variant retains its declaration identity")
                .local_index(),
            hir::Type::ImportedEnum(owner) => owner
                .variants
                .iter()
                .position(|variant| variant.identity == application.variant)
                .expect("a variant belongs to its declaring enum")
                as u32,
            _ => unreachable!("an enum variant retains its complete enum owner"),
        }
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
