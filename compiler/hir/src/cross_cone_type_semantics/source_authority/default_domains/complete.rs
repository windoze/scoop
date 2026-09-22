//! Atomic domain replay for all six reference kinds in one source transaction.
use super::*;

/// Complete target-domain equality. Operations, receivers, profiles and coverage
/// still require their own checks; this cannot authorize default execution.
#[derive(Debug)]
pub struct BoundNominalDefaultTargetDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    types: BoundNominalDefaultTypeDomainsV1<'b, 'd, 'p, 's, 'a, 'f>,
    values: BoundNominalDefaultValueDomainsV1<'b, 'd, 'p, 's, 'a, 'f>,
    callables: BoundNominalDefaultCallableDomainsV1<'b, 'd, 'p, 's, 'a, 'f>,
}
impl<'b, 'd, 'p, 's, 'a, 'f> BoundNominalDefaultTargetDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    pub const fn declarations(&self) -> &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f> {
        self.types.declarations()
    }
    pub const fn types(&self) -> &BoundNominalDefaultTypeDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
        &self.types
    }
    pub const fn values(&self) -> &BoundNominalDefaultValueDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
        &self.values
    }
    pub const fn callables(&self) -> &BoundNominalDefaultCallableDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
        &self.callables
    }
}
impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub fn bind_nominal_default_target_domains<'b, 'd, 'p, 's, 'a, 'f>(
        &self,
        declarations: &'b BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>,
        meter: &mut BudgetMeter,
    ) -> Result<
        BoundNominalDefaultTargetDomainsV1<'b, 'd, 'p, 's, 'a, 'f>,
        DefaultSourceTargetDomainBindingError,
    > {
        use DefaultSourceTargetDomainBindingError as Error;
        let types = self
            .bind_nominal_default_type_domains(declarations, meter)
            .map_err(Error::Type)?;
        let values = self
            .bind_nominal_default_value_domains(declarations, meter)
            .map_err(Error::Value)?;
        let callables = self
            .bind_nominal_default_callable_domains(declarations, meter)
            .map_err(Error::Callable)?;
        Ok(BoundNominalDefaultTargetDomainsV1 {
            types,
            values,
            callables,
        })
    }
}

#[derive(Debug)]
pub enum DefaultSourceTargetDomainBindingError {
    Type(DefaultSourceTypeDomainBindingError),
    Value(DefaultSourceValueDomainBindingError),
    Callable(DefaultSourceCallableDomainBindingError),
}
impl std::fmt::Display for DefaultSourceTargetDomainBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Type(error) => error.fmt(f),
            Self::Value(error) => error.fmt(f),
            Self::Callable(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for DefaultSourceTargetDomainBindingError {}
