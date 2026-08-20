use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

pub(crate) fn build(
    lowerer: &Lowerer,
    nominal_identities: &hir::HirNominalIdentities,
) -> Result<hir::HirObjectValueIdentities, PersistentObjectValueIdentityError> {
    hir::HirObjectValueIdentities::from_declarations(
        &lowerer.objects,
        &lowerer.object_types,
        &lowerer.singleton_values,
        nominal_identities,
    )
    .map_err(|error| {
        let location = error.object().map(|raw| {
            let object = hir::ObjectId::from_raw(raw.into());
            (
                lowerer.object_files.get(&object).copied().unwrap_or(0),
                lowerer.objects[object].span,
            )
        });
        let (file, span) = location.unwrap_or((0, Span { start: 0, end: 0 }));
        PersistentObjectValueIdentityError { file, span, error }
    })
}

#[derive(Debug)]
pub(crate) struct PersistentObjectValueIdentityError {
    file: usize,
    span: Span,
    error: hir::HirObjectValueIdentityError,
}

impl PersistentObjectValueIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentObjectValueIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent object-value identity: {}",
            self.error
        )
    }
}

impl std::error::Error for PersistentObjectValueIdentityError {}

#[cfg(test)]
mod tests;
