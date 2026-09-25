use super::*;
use DefaultSourceTypeAccessDemandV1 as Demand;

impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub(super) fn demand_domain(
        &self,
        demand: Demand<'_>,
        scope: &SignatureBinderScopeV1,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        match demand {
            Demand::Nominal(id) if id == self.core.unit().persistent() || id == self.any => {
                Ok(DefaultSourceAccessDomainV1::universal())
            }
            Demand::Nominal(id) => self.nominal_domain(SourceNominalId::Concrete(id), 0),
            Demand::NominalApplication { origin, arguments } => {
                self.nominal_domain(SourceNominalId::GenericTemplate(origin), arguments.len())
            }
            Demand::RawPointer { .. } => self.nominal_domain(
                SourceNominalId::GenericTemplate(self.core.ptr().persistent()),
                1,
            ),
            Demand::NativeFunctionPointer { .. } => self.nominal_domain(
                SourceNominalId::GenericTemplate(self.core.fun_ptr().persistent()),
                1,
            ),
            Demand::Binder { depth, index } => {
                scope
                    .validate(&SignatureTypeKey::Binder { depth, index })
                    .map_err(Error::Binder)?;
                Ok(DefaultSourceAccessDomainV1::universal())
            }
        }
    }
}
