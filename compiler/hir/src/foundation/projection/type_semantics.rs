use super::*;
use scoop_identity::ExactTypeKey;

impl CanonicalHirFoundation {
    /// M23-6 source authority needs the exact type of every generated nominal
    /// in the same sealed HIR, including an otherwise unused object backing
    /// class. Legacy producers keep their original projection unchanged.
    pub fn from_type_semantics_output(
        output: &crate::OrdinaryHirOutput<'_>,
    ) -> Result<Self, HirFoundationBuildError> {
        let mut foundation = Self::from_ordinary_output(output)?;
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
