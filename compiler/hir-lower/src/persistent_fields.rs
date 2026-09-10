use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

pub(crate) fn build(
    lowerer: &Lowerer,
    nominal_identities: &hir::HirNominalIdentities,
    property_identities: &hir::HirPropertyIdentities,
) -> Result<hir::HirFieldIdentities, PersistentFieldIdentityError> {
    hir::HirFieldIdentities::from_declarations(
        &lowerer.structs,
        &lowerer.classes,
        &lowerer.objects,
        &lowerer.class_fields,
        &lowerer.properties,
        &lowerer.delegate_storages,
        nominal_identities,
        property_identities,
    )
    .map_err(|error| {
        let location = error
            .structure()
            .and_then(|raw| {
                let structure = hir::StructId::from_raw(raw.into());
                ((raw as usize) < lowerer.structs.len()).then(|| {
                    (
                        lowerer.struct_files.get(&structure).copied().unwrap_or(0),
                        lowerer.structs[structure].span,
                    )
                })
            })
            .or_else(|| {
                error.class().and_then(|raw| {
                    let class = hir::ClassId::from_raw(raw.into());
                    ((raw as usize) < lowerer.classes.len()).then(|| {
                        (
                            lowerer.class_files.get(&class).copied().unwrap_or(0),
                            lowerer.classes[class].span,
                        )
                    })
                })
            })
            .or_else(|| {
                error.class_field().and_then(|raw| {
                    let field = hir::ClassFieldId::from_raw(raw.into());
                    ((raw as usize) < lowerer.class_fields.len()).then(|| {
                        let field = &lowerer.class_fields[field];
                        (
                            lowerer.class_files.get(&field.owner).copied().unwrap_or(0),
                            field.span,
                        )
                    })
                })
            });
        let (file, span) = location.unwrap_or((0, Span { start: 0, end: 0 }));
        PersistentFieldIdentityError { file, span, error }
    })
}

#[derive(Debug)]
pub(crate) struct PersistentFieldIdentityError {
    file: usize,
    span: Span,
    error: hir::HirFieldIdentityError,
}

impl PersistentFieldIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentFieldIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent field identity: {}",
            self.error
        )
    }
}

impl std::error::Error for PersistentFieldIdentityError {}

#[cfg(test)]
mod tests;
