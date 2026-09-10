use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

pub(crate) fn build(
    lowerer: &Lowerer,
    nominal_identities: &hir::HirNominalIdentities,
) -> Result<hir::HirEnumMemberIdentities, PersistentEnumMemberIdentityError> {
    hir::HirEnumMemberIdentities::from_declarations(&lowerer.enums, nominal_identities).map_err(
        |error| {
            let enumeration = hir::EnumId::from_raw(error.enumeration().into());
            PersistentEnumMemberIdentityError {
                file: lowerer.enum_files.get(&enumeration).copied().unwrap_or(0),
                span: lowerer.enums[enumeration].span,
                error,
            }
        },
    )
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
