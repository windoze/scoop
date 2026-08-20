use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

pub(crate) fn build(
    lowerer: &Lowerer,
    nominal_identities: &hir::HirNominalIdentities,
    property_identities: &hir::HirPropertyIdentities,
) -> Result<hir::HirInitializationUnitIdentities, PersistentInitializationUnitIdentityError> {
    hir::HirInitializationUnitIdentities::from_declarations(
        &lowerer.initialization_units,
        &lowerer.initialization_failure_roots,
        lowerer.functions.as_arena(),
        &lowerer.globals,
        &lowerer.objects,
        &lowerer.companion_relations,
        &lowerer.singleton_values,
        &lowerer.singleton_published_roots,
        &lowerer.properties,
        &lowerer.delegate_storages,
        &lowerer.generic_delegate_templates,
        nominal_identities,
        property_identities,
    )
    .map_err(|error| {
        let location = error.unit().and_then(|raw| {
            let unit = hir::InitializationUnitId::from_raw(raw.into());
            ((raw as usize) < lowerer.initialization_units.len()).then(|| {
                let declaration = &lowerer.initialization_units[unit];
                (
                    lowerer
                        .function_files
                        .get(&declaration.initializer)
                        .copied()
                        .unwrap_or(0),
                    declaration.span,
                )
            })
        });
        let (file, span) = location.unwrap_or((0, Span { start: 0, end: 0 }));
        PersistentInitializationUnitIdentityError { file, span, error }
    })
}

#[derive(Debug)]
pub(crate) struct PersistentInitializationUnitIdentityError {
    file: usize,
    span: Span,
    error: hir::HirInitializationUnitIdentityError,
}

impl PersistentInitializationUnitIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentInitializationUnitIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent initialization-unit identity: {}",
            self.error
        )
    }
}

impl std::error::Error for PersistentInitializationUnitIdentityError {}

#[cfg(test)]
mod tests;
