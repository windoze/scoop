use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

pub(crate) fn build(
    lowerer: &Lowerer,
    nominal_identities: &hir::HirNominalIdentities,
    core_types: hir::HirCoreTypeIdentityAuthority<'_>,
) -> Result<hir::HirTypeIdentities, PersistentTypeIdentityError> {
    hir::HirTypeIdentities::from_types(hir::HirTypeIdentityInputs {
        types: &lowerer.types,
        function_types: &lowerer.function_types,
        structs: &lowerer.structs,
        struct_applications: &lowerer.struct_applications,
        enums: &lowerer.enums,
        enum_applications: &lowerer.enum_applications,
        classes: &lowerer.classes,
        class_applications: &lowerer.class_applications,
        interfaces: &lowerer.interfaces,
        interface_applications: &lowerer.interface_applications,
        objects: &lowerer.objects,
        core_types,
        nominal_identities,
    })
    .map_err(|error| PersistentTypeIdentityError { error })
}

#[derive(Debug)]
pub(crate) struct PersistentTypeIdentityError {
    error: hir::HirTypeIdentityError,
}

impl PersistentTypeIdentityError {
    pub(crate) const fn file(&self) -> usize {
        0
    }

    pub(crate) const fn span(&self) -> Span {
        Span { start: 0, end: 0 }
    }
}

impl fmt::Display for PersistentTypeIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent type identity: {}",
            self.error
        )
    }
}

impl std::error::Error for PersistentTypeIdentityError {}

#[cfg(test)]
mod tests;
