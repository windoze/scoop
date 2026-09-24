use super::ImportedSemanticWorld;
use crate::{
    CanonicalNominalInterfacesV1, NominalExactLeafClassifierBuildError,
    NominalExactLeafClassifierV1,
};

impl ImportedSemanticWorld<'_> {
    /// Resolves nominal signatures using current declarations and the already
    /// validated dependency scope. Support declarations stay internal to this
    /// query; it does not introduce source lookup or machine capabilities.
    pub fn nominal_exact_leaf_classifier(
        &self,
        current: &CanonicalNominalInterfacesV1,
    ) -> Result<NominalExactLeafClassifierV1, NominalExactLeafClassifierBuildError> {
        NominalExactLeafClassifierV1::try_from_nominal_interfaces(
            current.records().iter().chain(
                self.providers
                    .iter()
                    .flat_map(|provider| provider.interface().nominal_interfaces().records()),
            ),
        )
    }
}
