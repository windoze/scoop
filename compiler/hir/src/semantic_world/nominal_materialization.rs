//! Necessary source conditions use the actual provider's shared declarations.

use scoop_identity::{ConeIdentity, PersistentTypeId};

use super::ImportedSemanticWorld;
use crate::{NominalMaterializationClosure, NominalMaterializationClosureError, SourceNominalId};

impl ImportedSemanticWorld<'_> {
    /// Checks one provider's source declaration closure by typed identity.
    /// This does not establish complete dependency layout or machine-use
    /// eligibility, and exposes no name enumeration for support providers.
    pub fn has_materializable_nominal_source(
        &self,
        provider: ConeIdentity,
        source: PersistentTypeId,
    ) -> Result<bool, NominalMaterializationClosureError> {
        let provider = self
            .provider(provider)
            .ok_or(NominalMaterializationClosureError::MissingNominal(source))?;
        let interface = provider.interface();
        let nominals = interface.nominal_interfaces();

        if nominals
            .declaration(SourceNominalId::Concrete(source))
            .is_none()
        {
            return Err(NominalMaterializationClosureError::MissingNominal(source));
        }
        let closure = NominalMaterializationClosure::from_declarations(
            nominals,
            interface.callable_interfaces(),
        )?;

        Ok(closure.contains(source))
    }
}
