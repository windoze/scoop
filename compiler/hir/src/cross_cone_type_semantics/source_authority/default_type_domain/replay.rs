use super::*;
use DefaultSourceTypeAccessDemandV1 as Demand;

impl DefaultSourceTypeDomainsV1<'_, '_, '_, '_> {
    pub(super) fn demand_domain(
        &self,
        demand: Demand<'_>,
        scope: &SignatureBinderScopeV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        meter.charge_work(1, path)?;
        match demand {
            Demand::Nominal(id) if id == self.core.unit().persistent() || id == self.any => {
                Ok(DefaultSourceAccessDomainV1::universal())
            }
            Demand::Nominal(id) => {
                self.nominal_domain(SourceNominalId::Concrete(id), 0, meter, path)
            }
            Demand::NominalApplication { origin, arguments } => self.nominal_domain(
                SourceNominalId::GenericTemplate(origin),
                arguments.len(),
                meter,
                path,
            ),
            Demand::RawPointer { .. } => self.nominal_domain(
                SourceNominalId::GenericTemplate(self.core.ptr().persistent()),
                1,
                meter,
                path,
            ),
            Demand::NativeFunctionPointer { .. } => self.nominal_domain(
                SourceNominalId::GenericTemplate(self.core.fun_ptr().persistent()),
                1,
                meter,
                path,
            ),
            Demand::Binder { depth, index } => {
                meter.check_semantic_depth(u64::from(scope.available_depths()), path)?;
                scope
                    .validate(&SignatureTypeKey::Binder { depth, index })
                    .map_err(Error::Binder)?;
                Ok(DefaultSourceAccessDomainV1::universal())
            }
        }
    }
}
