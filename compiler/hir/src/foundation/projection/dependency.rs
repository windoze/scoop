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
        let declared_source_types = foundation
            .types
            .iter()
            .map(CborIdentityRecord::id)
            .chain(
                foundation
                    .generated_types
                    .iter()
                    .map(CborIdentityRecord::id),
            )
            .collect::<std::collections::BTreeSet<_>>();
        let declared_generic_types = foundation
            .generic_types
            .iter()
            .map(CborIdentityRecord::id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut external_source_types = std::collections::BTreeSet::new();
        let mut external_generic_types = std::collections::BTreeSet::new();
        for record in &foundation.exact_types {
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
        for record in &foundation.native_boundary_types {
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
        foundation.set_external_source_types(external_source_types.into_iter().collect())?;
        foundation.set_external_generic_types(external_generic_types.into_iter().collect())?;
        super::call_points::complete(output, &mut foundation)?;
        Ok(foundation)
    }
}
