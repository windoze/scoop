//! Necessary source conditions use the actual provider's shared declarations.

use scoop_identity::{ConeIdentity, PersistentTypeId};
use scoop_wire::{BudgetMeter, WirePath};

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
        meter: &mut BudgetMeter,
    ) -> Result<bool, NominalMaterializationClosureError> {
        let path = WirePath::root();
        meter.charge_work(1 + u64::from(self.providers.len().max(1).ilog2()), &path)?;
        let provider = self
            .provider(provider)
            .ok_or(NominalMaterializationClosureError::MissingNominal(source))?;
        let interface = provider.interface();
        let nominals = interface.nominal_interfaces();
        meter.charge_work(
            1 + u64::from(nominals.declaration_count().max(1).ilog2()),
            &path,
        )?;
        if nominals
            .declaration(SourceNominalId::Concrete(source))
            .is_none()
        {
            return Err(NominalMaterializationClosureError::MissingNominal(source));
        }
        let closure = NominalMaterializationClosure::from_declarations(
            nominals,
            interface.callable_interfaces(),
            meter,
        )?;
        meter.charge_work(1 + u64::from(closure.sources().len().max(1).ilog2()), &path)?;
        Ok(closure.contains(source))
    }
}
