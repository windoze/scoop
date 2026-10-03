use super::*;
use scoop_identity::ExactTypeKey;

impl CanonicalHirFoundation {
    /// Completes exact identities for generated nominal representations.
    /// Shared interface construction supplies all required source positions.
    pub fn from_type_semantics_output(
        output: &crate::DependencyHirOutput,
    ) -> Result<Self, HirFoundationBuildError> {
        let mut foundation = Self::from_dependency_output(output)?;
        let mut exacts = foundation
            .exact_types
            .iter()
            .map(|record| (record.id(), record.clone()))
            .collect::<BTreeMap<_, _>>();
        for generated in &foundation.generated_types {
            let key = ExactTypeKey::Nominal(generated.id());
            let exact = CborIdentityRecord::from_key(key).map_err(|error| {
                HirFoundationBuildError::IdentityDerivation {
                    table: HirFoundationTable::ExactType,
                    reason: error.to_string(),
                }
            })?;
            insert_identity(&mut exacts, &exact, HirFoundationTable::ExactType)?;
        }
        foundation.set_exact_types(exacts.into_values().collect())?;
        Ok(foundation)
    }
}
