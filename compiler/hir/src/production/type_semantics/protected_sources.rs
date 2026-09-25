//! Complete protected declaration candidates, independently selected from HIR.
use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_errors::invalid;
use super::nested_sources::{project_protocols, project_record, resources};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId, PersistentPropertyId};
use std::collections::BTreeSet;

mod inventory;
mod records;

/// Independently selected source candidates. This does not grant default,
/// inheritance-selection, or executable materialization authority.
#[derive(Clone, Debug)]
pub struct ProtectedDeclarationSourceProductionV1 {
    required: CanonicalProtectedDeclarationRefsV1,
    declarations: CanonicalProtectedDeclarationInterfacesV1,
    protocols: CanonicalProtectedCallableSourceInterfacesV1,
}
impl ProtectedDeclarationSourceProductionV1 {
    pub fn from_export_hir(output: &ExportHirOutput) -> Result<Self, Error> {
        let required = CanonicalProtectedDeclarationRefsV1::from_export_hir(output)?;
        let (declarations, owners) = records::project(output, &required)?;
        let protocols = project_protocols(output.module(), owners)?;

        if !required.values().iter().copied().eq(declarations
            .records()
            .iter()
            .map(ProtectedDeclarationInterfaceV1::reference))
        {
            return Err(invalid(
                "protected source projection differs from required declarations",
            ));
        }
        Ok(Self {
            required,
            declarations,
            protocols,
        })
    }
    pub const fn required(&self) -> &CanonicalProtectedDeclarationRefsV1 {
        &self.required
    }
    pub const fn declarations(&self) -> &CanonicalProtectedDeclarationInterfacesV1 {
        &self.declarations
    }
    pub const fn protocols(&self) -> &CanonicalProtectedCallableSourceInterfacesV1 {
        &self.protocols
    }
    pub fn into_parts(
        self,
    ) -> (
        CanonicalProtectedDeclarationRefsV1,
        CanonicalProtectedDeclarationInterfacesV1,
        CanonicalProtectedCallableSourceInterfacesV1,
    ) {
        (self.required, self.declarations, self.protocols)
    }
}
