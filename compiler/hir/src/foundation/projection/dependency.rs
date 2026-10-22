use super::*;

impl CanonicalHirFoundation {
    /// Builds a current-Cone HIR foundation with the external nominal identities
    /// referenced by its exact types.
    pub fn from_dependency_output(
        output: &crate::DependencyHirOutput,
    ) -> Result<Self, HirFoundationBuildError> {
        let hir = output.output();
        let mut foundation =
            Self::from_modules(&hir.export, &hir.local, &hir.native_boundary_types)?;
        foundation.complete_external_nominals()?;
        super::call_points::complete(output, &mut foundation)?;
        Ok(foundation)
    }

    pub(super) fn complete_external_nominals(&mut self) -> Result<(), HirFoundationBuildError> {
        let declared_source_types = self
            .types
            .iter()
            .map(CborIdentityRecord::id)
            .chain(self.generated_types.iter().map(CborIdentityRecord::id))
            .collect::<std::collections::BTreeSet<_>>();
        let declared_generic_types = self
            .generic_types
            .iter()
            .map(CborIdentityRecord::id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut external_source_types = std::collections::BTreeSet::new();
        let mut external_generic_types = std::collections::BTreeSet::new();
        for record in &self.exact_types {
            match record.key() {
                ExactTypeKey::Nominal(source) if !declared_source_types.contains(source) => {
                    external_source_types.insert(*source);
                }
                ExactTypeKey::NominalApplication { origin, .. }
                    if !declared_generic_types.contains(origin) =>
                {
                    external_generic_types.insert(*origin);
                }
                ExactTypeKey::Nominal(_)
                | ExactTypeKey::NominalApplication { .. }
                | ExactTypeKey::Tuple(_)
                | ExactTypeKey::Function { .. }
                | ExactTypeKey::RawPointer(_)
                | ExactTypeKey::NativeFunctionPointer { .. } => {}
            }
        }
        for record in &self.native_boundary_types {
            match record.owner() {
                crate::NativeBoundaryNominalOwner::Concrete(source)
                    if !declared_source_types.contains(&source) =>
                {
                    external_source_types.insert(source);
                }
                crate::NativeBoundaryNominalOwner::GenericTemplate(origin)
                    if !declared_generic_types.contains(&origin) =>
                {
                    external_generic_types.insert(origin);
                }
                crate::NativeBoundaryNominalOwner::Concrete(_)
                | crate::NativeBoundaryNominalOwner::GenericTemplate(_) => {}
            }
        }
        self.set_external_source_types(external_source_types.into_iter().collect())?;
        self.set_external_generic_types(external_generic_types.into_iter().collect())
    }
}
